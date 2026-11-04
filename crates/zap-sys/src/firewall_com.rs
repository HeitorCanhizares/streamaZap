//! Acesso ao firewall via COM `INetFwPolicy2` (somente Windows).

use std::path::Path;

use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::{E_ACCESSDENIED, VARIANT_BOOL};
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, INetFwRules, NetFwPolicy2, NetFwRule, NET_FW_ACTION_ALLOW, NET_FW_ACTION_BLOCK,
    NET_FW_PROFILE_TYPE2, NET_FW_RULE_DIR_IN, NET_FW_RULE_DIR_OUT,
};
use windows::Win32::Networking::NetworkListManager::{
    INetworkListManager, NetworkListManager, NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED, NLM_NETWORK_CATEGORY_PRIVATE,
    NLM_NETWORK_CATEGORY_PUBLIC,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IDispatch, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Ole::IEnumVARIANT;
use windows::Win32::System::Variant::{VARIANT, VT_DISPATCH};

use crate::firewall::{Acao, Coleta, Direcao, PerfilBruto, PlanoCorrecao, RegraBruta, Resultado, NOMES_PERFIS};
use crate::{ErroSys, Result};

const NOME_UDP: &str = "streamaZap (UDP)";
const NOME_TCP: &str = "streamaZap (TCP)";

/// Inicializa o COM na thread. S_FALSE / RPC_E_CHANGED_MODE significam "já inicializado": ignoramos
/// e nunca chamamos `CoUninitialize`, pois não temos como saber se a inicialização foi nossa.
fn iniciar_com() {
    // SAFETY: chamada simples de inicialização do COM, sem ponteiros.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

/// Converte erro do COM; acesso negado vira `SemPermissao`.
fn mapear(e: windows::core::Error, contexto: &str) -> ErroSys {
    if e.code() == E_ACCESSDENIED {
        ErroSys::SemPermissao
    } else {
        ErroSys::Outro(format!("{contexto}: {e}"))
    }
}

fn politica() -> Result<INetFwPolicy2> {
    iniciar_com();
    // SAFETY: CLSID válido; o COM já foi inicializado nesta thread.
    unsafe { CoCreateInstance::<_, INetFwPolicy2>(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| mapear(e, "criar NetFwPolicy2"))
}

/// Expande `%VAR%` usando o ambiente do processo; variável desconhecida fica como está.
fn expandir_variaveis(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(ini) = resto.find('%') {
        saida.push_str(&resto[..ini]);
        let depois = &resto[ini + 1..];
        match depois.find('%') {
            Some(fim) => {
                let nome = &depois[..fim];
                match std::env::var(nome) {
                    Ok(v) if !nome.is_empty() => saida.push_str(&v),
                    _ => {
                        saida.push('%');
                        saida.push_str(nome);
                        saida.push('%');
                    }
                }
                resto = &depois[fim + 1..];
            }
            None => {
                saida.push('%');
                resto = depois;
                break;
            }
        }
    }
    saida.push_str(resto);
    saida
}

fn bstr_para_string(b: &BSTR) -> String {
    String::from_utf16_lossy(b)
}

fn ler_regra(r: &INetFwRule) -> Option<RegraBruta> {
    // SAFETY: `r` é uma interface COM válida; os getters só escrevem em saídas locais.
    unsafe {
        let nome = r.Name().map(|b| bstr_para_string(&b)).unwrap_or_default();
        let caminho = r.ApplicationName().map(|b| bstr_para_string(&b)).unwrap_or_default();
        let direcao = if r.Direction().ok()? == NET_FW_RULE_DIR_OUT {
            Direcao::Saida
        } else {
            Direcao::Entrada
        };
        let acao = if r.Action().ok()? == NET_FW_ACTION_BLOCK {
            Acao::Bloquear
        } else {
            Acao::Permitir
        };
        let perfis = r.Profiles().unwrap_or(0) as u32;
        let ativa = r.Enabled().map(|b| b.as_bool()).unwrap_or(false);
        Some(RegraBruta {
            nome,
            caminho_app: expandir_variaveis(&caminho),
            direcao,
            acao,
            perfis,
            ativa,
        })
    }
}

/// Lê todas as regras e os perfis (`CoCreateInstance(NetFwPolicy2)`), expandindo variáveis de
/// ambiente nos caminhos. Inicializa o COM na thread se preciso. A categoria do Radmin é lida
/// por `categoria_radmin_windows` (outra função, mesmo arquivo).
pub(crate) fn coletar() -> Result<Coleta> {
    let policy = politica()?;
    let mut regras = Vec::new();

    // SAFETY: todas as interfaces vêm do COM e vivem até o fim do bloco; o VARIANT devolvido por
    // `Next` é liberado no Drop (VariantClear); usamos só um clone do IDispatch.
    unsafe {
        let rules = policy.Rules().map_err(|e| mapear(e, "ler regras"))?;
        let enumerador = rules
            ._NewEnum()
            .and_then(|u| u.cast::<IEnumVARIANT>())
            .map_err(|e| mapear(e, "enumerar regras"))?;
        loop {
            let mut item = [VARIANT::default()];
            let mut buscados = 0u32;
            let hr = enumerador.Next(&mut item, &mut buscados);
            if hr.is_err() || buscados == 0 {
                break;
            }
            let [v] = item;
            let interno = &v.Anonymous.Anonymous;
            if interno.vt == VT_DISPATCH {
                // Clone (AddRef): o VARIANT continua dono da referência original e a libera no Drop.
                let disp: Option<IDispatch> = (*interno.Anonymous.pdispVal).clone();
                if let Some(regra) = disp.and_then(|d| d.cast::<INetFwRule>().ok()) {
                    if let Some(r) = ler_regra(&regra) {
                        regras.push(r);
                    }
                }
            }
            if hr.0 != 0 {
                // S_FALSE: acabou o enumerador.
                break;
            }
        }
    }

    let tipos = [1, 2, 4];
    let mut perfis = [PerfilBruto::default(); 3];
    for (i, t) in tipos.iter().enumerate() {
        let tipo = NET_FW_PROFILE_TYPE2(*t);
        // SAFETY: `policy` é válida; os getters não têm ponteiros de entrada.
        unsafe {
            perfis[i].ativo = policy.get_FirewallEnabled(tipo).map(|b| b.as_bool()).unwrap_or(false);
            perfis[i].bloquear_tudo_entrada = policy
                .get_BlockAllInboundTraffic(tipo)
                .map(|b| b.as_bool())
                .unwrap_or(false);
        }
    }

    Ok(Coleta {
        regras,
        perfis,
        categoria_radmin: categoria_radmin_windows(),
    })
}

fn remover_regra(rules: &INetFwRules, nome: &str) -> Result<()> {
    // SAFETY: `rules` é válida e o BSTR vive durante a chamada.
    unsafe { rules.Remove(&BSTR::from(nome)) }.map_err(|e| mapear(e, "remover regra"))
}

/// Cria uma regra de entrada permitindo `exe` no protocolo e portas dados.
fn criar_regra(rules: &INetFwRules, nome: &str, exe: &str, protocolo: i32, portas: &str) -> Result<()> {
    iniciar_com();
    // SAFETY: CLSID válido, COM inicializado; setters recebem BSTRs que vivem durante a chamada.
    unsafe {
        let regra = CoCreateInstance::<_, INetFwRule>(&NetFwRule, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| mapear(e, "criar NetFwRule"))?;
        let ok = |e| mapear(e, "configurar regra");
        regra.SetName(&BSTR::from(nome)).map_err(ok)?;
        regra.SetApplicationName(&BSTR::from(exe)).map_err(ok)?;
        regra.SetDirection(NET_FW_RULE_DIR_IN).map_err(ok)?;
        regra.SetAction(NET_FW_ACTION_ALLOW).map_err(ok)?;
        regra.SetProfiles(0x7FFF_FFFF).map_err(ok)?;
        regra.SetProtocol(protocolo).map_err(ok)?;
        regra.SetLocalPorts(&BSTR::from(portas)).map_err(ok)?;
        regra.SetGrouping(&BSTR::from("streamaZap")).map_err(ok)?;
        regra.SetEnabled(VARIANT_BOOL(-1)).map_err(ok)?;
        rules.Add(&regra).map_err(|e| mapear(e, "adicionar regra"))
    }
}

/// Aplica o plano: remove regras pelo nome, cria "streamaZap (UDP)" e "streamaZap (TCP)" de entrada
/// para `exe` em todos os perfis (UDP portas 47800-47820; TCP 47801), e desliga
/// `BlockAllInboundTraffic` nos perfis do plano.
pub(crate) fn aplicar(exe: &Path, plano: &PlanoCorrecao, removidas_alvo: u32) -> Result<Resultado> {
    let policy = politica()?;
    // SAFETY: `policy` é válida.
    let rules = unsafe { policy.Rules() }.map_err(|e| mapear(e, "ler regras"))?;

    let mut removidas = 0u32;
    for nome in &plano.remover {
        match remover_regra(&rules, nome) {
            Ok(()) => removidas += 1,
            Err(ErroSys::SemPermissao) => return Err(ErroSys::SemPermissao),
            Err(e) => tracing::warn!("falha ao remover a regra '{nome}': {e}"),
        }
    }
    // Remove as nossas antigas para não duplicar ao repetir; ausência não é erro.
    for nome in [NOME_UDP, NOME_TCP] {
        if let Err(ErroSys::SemPermissao) = remover_regra(&rules, nome) {
            return Err(ErroSys::SemPermissao);
        }
    }

    let exe_str = exe.to_string_lossy();
    criar_regra(&rules, NOME_UDP, &exe_str, 17, "47800-47820")?;
    criar_regra(&rules, NOME_TCP, &exe_str, 6, "47801")?;

    let mut liberados = Vec::new();
    for i in &plano.liberar_perfis {
        let Some(nome) = NOMES_PERFIS.get(*i) else { continue };
        let tipo = NET_FW_PROFILE_TYPE2(1 << *i);
        // SAFETY: `policy` é válida.
        unsafe { policy.put_BlockAllInboundTraffic(tipo, VARIANT_BOOL(0)) }.map_err(|e| mapear(e, "liberar perfil"))?;
        liberados.push((*nome).to_string());
    }

    Ok(Resultado {
        ok: true,
        mensagem: format!(
            "ok removidas={removidas} (previstas {removidas_alvo}) perfis_liberados={}",
            liberados.join(",")
        ),
        removidas,
        perfis_liberados: liberados,
    })
}

/// Categoria (`Publica`/`Privada`/`Dominio`) da rede cujo adaptador contém "Radmin" no nome.
pub(crate) fn categoria_radmin_windows() -> Option<String> {
    iniciar_com();
    // SAFETY: CLSID válido, COM inicializado; interfaces vivem durante o bloco.
    unsafe {
        let gerente =
            CoCreateInstance::<_, INetworkListManager>(&NetworkListManager, None, CLSCTX_INPROC_SERVER).ok()?;
        let conexoes = gerente.GetNetworkConnections().ok()?;
        loop {
            let mut item = [None];
            let mut buscados = 0u32;
            if conexoes.Next(&mut item, Some(&mut buscados)).is_err() || buscados == 0 {
                return None;
            }
            let conexao = item[0].take()?;
            let Ok(rede) = conexao.GetNetwork() else { continue };
            let nome = rede.GetName().map(|b| bstr_para_string(&b)).unwrap_or_default();
            if !nome.to_lowercase().contains("radmin") {
                continue;
            }
            let cat = rede.GetCategory().ok()?;
            return if cat == NLM_NETWORK_CATEGORY_PUBLIC {
                Some("Publica".to_string())
            } else if cat == NLM_NETWORK_CATEGORY_PRIVATE {
                Some("Privada".to_string())
            } else if cat == NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED {
                Some("Dominio".to_string())
            } else {
                None
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coletar_funciona_sem_admin() {
        let c = coletar().expect("coletar deve funcionar sem admin");
        assert_eq!(c.perfis.len(), 3);
    }

    #[test]
    fn categoria_radmin_nao_entra_em_panico() {
        let _ = categoria_radmin_windows();
    }

    #[test]
    fn expande_variaveis() {
        assert_eq!(expandir_variaveis("sem vars"), "sem vars");
        assert_eq!(expandir_variaveis("%ZAP_NAO_EXISTE_XYZ%\\a"), "%ZAP_NAO_EXISTE_XYZ%\\a");
        assert_eq!(expandir_variaveis("50%"), "50%");
    }

    #[test]
    #[ignore = "precisa de administrador"]
    fn aplicar_cria_e_repete_sem_duplicar() {
        let plano = PlanoCorrecao {
            remover: vec![],
            liberar_perfis: vec![],
        };
        let exe = Path::new("C:\\temp\\zap-teste.exe");
        let r = aplicar(exe, &plano, 0).expect("aplicar");
        assert!(r.ok);
        let r2 = aplicar(exe, &plano, 0).expect("aplicar de novo");
        assert!(r2.ok);
        let policy = politica().expect("politica");
        // SAFETY: limpeza das regras de teste.
        let rules = unsafe { policy.Rules() }.expect("rules");
        remover_regra(&rules, NOME_UDP).expect("remover udp");
        remover_regra(&rules, NOME_TCP).expect("remover tcp");
    }
}
