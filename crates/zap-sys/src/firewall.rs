//! Firewall do Windows: tipos, interpretação pura das regras e funções públicas
//! (`status`, `corrigir`, `corrigir_elevado`). A coleta/aplicação via COM fica em `firewall_com.rs`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Nomes dos perfis na ordem `[dominio, privado, publico]` (bits 1, 2 e 4 de `Profiles`).
pub const NOMES_PERFIS: [&str; 3] = ["dominio", "privado", "publico"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direcao {
    Entrada,
    Saida,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Acao {
    Bloquear,
    Permitir,
}

/// Regra do firewall como lida do COM (dados brutos, sem interpretação).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegraBruta {
    pub nome: String,
    /// Caminho do aplicativo já com variáveis de ambiente expandidas (pode ser vazio).
    pub caminho_app: String,
    pub direcao: Direcao,
    pub acao: Acao,
    /// Máscara `NET_FW_PROFILE2_*`: 1 domínio, 2 privado, 4 público (0x7FFFFFFF = todos).
    pub perfis: u32,
    pub ativa: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PerfilBruto {
    pub ativo: bool,
    /// "Bloquear todas as conexões de entrada, incluindo as da lista de permitidos".
    pub bloquear_tudo_entrada: bool,
}

/// Tudo que o COM devolve, antes de interpretar. `regras` são TODAS as regras do sistema.
#[derive(Debug, Clone, Default)]
pub struct Coleta {
    pub regras: Vec<RegraBruta>,
    /// `[dominio, privado, publico]`
    pub perfis: [PerfilBruto; 3],
    /// Categoria da rede do adaptador Radmin: "Publica", "Privada", "Dominio".
    pub categoria_radmin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegraResumo {
    pub nome: String,
    pub direcao: Direcao,
    pub acao: Acao,
    pub perfis: u32,
    pub ativa: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirewallStatus {
    pub suportado: bool,
    /// Existe regra de entrada ativa de BLOQUEIO para o executável.
    pub bloqueado: bool,
    /// Existe regra de entrada ativa de permissão cobrindo os perfis privado e público.
    pub liberado: bool,
    /// "Bloquear todas as conexões de entrada" ligado, por perfil `[dominio, privado, publico]`
    /// (só conta perfil ativo).
    pub bloquear_tudo_por_perfil: [bool; 3],
    pub categoria_radmin: Option<String>,
    pub regras: Vec<RegraResumo>,
    pub erro: Option<String>,
}

impl FirewallStatus {
    /// Status para sistemas sem firewall do Windows.
    pub fn nao_suportado() -> Self {
        Self {
            suportado: false,
            bloqueado: false,
            liberado: false,
            bloquear_tudo_por_perfil: [false; 3],
            categoria_radmin: None,
            regras: Vec::new(),
            erro: None,
        }
    }

    /// Precisa de correção (porte de `needs_fix`).
    pub fn precisa_corrigir(&self) -> bool {
        self.suportado
            && self.erro.is_none()
            && (self.bloqueado || !self.liberado || self.bloquear_tudo_por_perfil.iter().any(|b| *b))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resultado {
    pub ok: bool,
    pub mensagem: String,
    pub removidas: u32,
    /// Nomes (`NOMES_PERFIS`) dos perfis em que "Bloquear tudo" foi desligado.
    pub perfis_liberados: Vec<String>,
}

/// Compara caminhos do Windows sem diferenciar maiúsculas e normalizando `/` e `\`.
pub fn caminho_igual(a: &str, b: &str) -> bool {
    // Vazio nunca casa: evita que "" seja igual a "" e marque regras sem caminho.
    if a.is_empty() || b.is_empty() {
        return false;
    }
    normalizar_caminho(a) == normalizar_caminho(b)
}

/// Forma canônica para comparação: minúsculas, `/` vira `\` e barras finais são removidas.
fn normalizar_caminho(c: &str) -> String {
    c.replace('/', "\\").to_lowercase().trim_end_matches('\\').to_string()
}

/// Bits de `Profiles` para privado (2) e público (4): precisam estar ambos marcados para liberar.
const MASCARA_PRIVADO_PUBLICO: u32 = 2 | 4;

/// Regras da `coleta` que pertencem ao executável `exe` (pelo caminho, via `caminho_igual`).
pub fn regras_do_exe<'a>(coleta: &'a Coleta, exe: &Path) -> Vec<&'a RegraBruta> {
    let alvo = exe.to_string_lossy();
    coleta
        .regras
        .iter()
        .filter(|r| caminho_igual(&r.caminho_app, &alvo))
        .collect()
}

/// Interpreta a coleta (função pura): bloqueado, liberado, bloquear-tudo, resumo das regras.
pub fn interpretar(exe: &Path, coleta: &Coleta) -> FirewallStatus {
    let mine = regras_do_exe(coleta, exe);
    let entrada_ativas: Vec<&RegraBruta> = mine
        .iter()
        .copied()
        .filter(|r| r.ativa && r.direcao == Direcao::Entrada)
        .collect();

    let bloqueado = entrada_ativas.iter().any(|r| r.acao == Acao::Bloquear);
    let liberado = entrada_ativas
        .iter()
        .any(|r| r.acao == Acao::Permitir && (r.perfis & MASCARA_PRIVADO_PUBLICO) == MASCARA_PRIVADO_PUBLICO);
    let bloquear_tudo_por_perfil = coleta.perfis.map(|p| p.ativo && p.bloquear_tudo_entrada);

    FirewallStatus {
        suportado: true,
        bloqueado,
        liberado,
        bloquear_tudo_por_perfil,
        categoria_radmin: coleta.categoria_radmin.clone(),
        regras: mine
            .iter()
            .map(|r| RegraResumo {
                nome: r.nome.clone(),
                direcao: r.direcao,
                acao: r.acao,
                perfis: r.perfis,
                ativa: r.ativa,
            })
            .collect(),
        erro: None,
    }
}

/// Perfil (índice em `NOMES_PERFIS`) que corresponde à categoria do Radmin
/// ("Dominio" -> 0, "Privada" -> 1, "Publica" -> 2); `None` se desconhecida.
pub fn perfil_da_categoria(categoria: &str) -> Option<usize> {
    match categoria.trim().to_lowercase().as_str() {
        "dominio" | "domínio" | "domainauthenticated" | "domain" => Some(0),
        "privada" | "private" => Some(1),
        "publica" | "pública" | "public" => Some(2),
        _ => None,
    }
}

/// Plano de correção (função pura): nomes das regras a remover (bloqueios do exe) e perfis em que
/// desligar "Bloquear tudo" (só o do Radmin, se estiver ligado e ativo).
pub fn planejar_correcao(exe: &Path, coleta: &Coleta) -> PlanoCorrecao {
    // Remove todo bloqueio do exe, ativo ou não, sem repetir nomes.
    let mut remover: Vec<String> = Vec::new();
    for r in regras_do_exe(coleta, exe) {
        if r.acao == Acao::Bloquear && !remover.contains(&r.nome) {
            remover.push(r.nome.clone());
        }
    }

    // Só desliga "Bloquear tudo" do perfil do Radmin, e só se estiver ligado e ativo.
    let liberar_perfis: Vec<usize> = coleta
        .categoria_radmin
        .as_deref()
        .and_then(perfil_da_categoria)
        .filter(|&i| coleta.perfis.get(i).is_some_and(|p| p.ativo && p.bloquear_tudo_entrada))
        .into_iter()
        .collect();

    PlanoCorrecao {
        remover,
        liberar_perfis,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanoCorrecao {
    /// Nomes das regras de bloqueio do exe a remover.
    pub remover: Vec<String>,
    /// Índices (em `NOMES_PERFIS`) onde desligar "Bloquear tudo".
    pub liberar_perfis: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\Apps\Radmin\rserver.exe";

    fn regra(nome: &str, caminho: &str, direcao: Direcao, acao: Acao, perfis: u32, ativa: bool) -> RegraBruta {
        RegraBruta {
            nome: nome.into(),
            caminho_app: caminho.into(),
            direcao,
            acao,
            perfis,
            ativa,
        }
    }

    fn coleta(regras: Vec<RegraBruta>) -> Coleta {
        Coleta {
            regras,
            ..Coleta::default()
        }
    }

    #[test]
    fn caminho_igual_ignora_maiusculas_e_barras() {
        assert!(caminho_igual(r"C:\Apps\App.exe", "c:/apps/app.EXE"));
        assert!(caminho_igual(r"C:\Apps\", r"c:\apps"));
        assert!(!caminho_igual(r"C:\Apps\a.exe", r"C:\Apps\b.exe"));
    }

    #[test]
    fn caminho_igual_vazio_e_falso() {
        assert!(!caminho_igual("", ""));
        assert!(!caminho_igual("", r"C:\a.exe"));
        assert!(!caminho_igual(r"C:\a.exe", ""));
    }

    #[test]
    fn interpretar_sem_regras_nao_liberado() {
        let st = interpretar(Path::new(EXE), &Coleta::default());
        assert!(st.suportado);
        assert!(st.erro.is_none());
        assert!(!st.bloqueado);
        assert!(!st.liberado);
        assert_eq!(st.bloquear_tudo_por_perfil, [false; 3]);
        assert!(st.regras.is_empty());
    }

    #[test]
    fn interpretar_bloqueio_ativo_marca_bloqueado() {
        let c = coleta(vec![regra(
            "Bloq",
            EXE,
            Direcao::Entrada,
            Acao::Bloquear,
            0x7FFF_FFFF,
            true,
        )]);
        let st = interpretar(Path::new(EXE), &c);
        assert!(st.bloqueado);
        assert_eq!(st.regras.len(), 1);
    }

    #[test]
    fn interpretar_bloqueio_desativado_nao_conta() {
        let c = coleta(vec![regra(
            "Bloq",
            EXE,
            Direcao::Entrada,
            Acao::Bloquear,
            0x7FFF_FFFF,
            false,
        )]);
        let st = interpretar(Path::new(EXE), &c);
        assert!(!st.bloqueado);
        // Ainda aparece no resumo, mesmo inativa.
        assert_eq!(st.regras.len(), 1);
    }

    #[test]
    fn interpretar_permissao_so_publico_nao_libera() {
        let c = coleta(vec![regra("Perm", EXE, Direcao::Entrada, Acao::Permitir, 4, true)]);
        let st = interpretar(Path::new(EXE), &c);
        assert!(!st.liberado);
    }

    #[test]
    fn interpretar_permissao_privado_e_publico_libera() {
        let c = coleta(vec![regra("Perm", EXE, Direcao::Entrada, Acao::Permitir, 6, true)]);
        assert!(interpretar(Path::new(EXE), &c).liberado);

        let c = coleta(vec![regra(
            "Perm",
            EXE,
            Direcao::Entrada,
            Acao::Permitir,
            0x7FFF_FFFF,
            true,
        )]);
        assert!(interpretar(Path::new(EXE), &c).liberado);
    }

    #[test]
    fn interpretar_ignora_regras_de_outro_exe() {
        let c = coleta(vec![regra(
            "Outro",
            r"C:\Outro\x.exe",
            Direcao::Entrada,
            Acao::Bloquear,
            0x7FFF_FFFF,
            true,
        )]);
        let st = interpretar(Path::new(EXE), &c);
        assert!(!st.bloqueado);
        assert!(st.regras.is_empty());
    }

    #[test]
    fn interpretar_bloquear_tudo_so_perfil_ativo() {
        let c = Coleta {
            perfis: [
                PerfilBruto {
                    ativo: false,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: false,
                },
            ],
            ..Coleta::default()
        };
        let st = interpretar(Path::new(EXE), &c);
        assert_eq!(st.bloquear_tudo_por_perfil, [false, true, false]);
    }

    #[test]
    fn perfil_da_categoria_variantes() {
        assert_eq!(perfil_da_categoria("Dominio"), Some(0));
        assert_eq!(perfil_da_categoria("domínio"), Some(0));
        assert_eq!(perfil_da_categoria("Privada"), Some(1));
        assert_eq!(perfil_da_categoria("private"), Some(1));
        assert_eq!(perfil_da_categoria("Publica"), Some(2));
        assert_eq!(perfil_da_categoria("Pública"), Some(2));
        assert_eq!(perfil_da_categoria("x"), None);
    }

    #[test]
    fn planejar_remove_bloqueios_sem_repetir() {
        let c = coleta(vec![
            regra("A", EXE, Direcao::Entrada, Acao::Bloquear, 0x7FFF_FFFF, true),
            regra("A", EXE, Direcao::Saida, Acao::Bloquear, 0x7FFF_FFFF, false),
            regra("B", EXE, Direcao::Entrada, Acao::Bloquear, 0x7FFF_FFFF, false),
            regra("C", EXE, Direcao::Entrada, Acao::Permitir, 6, true),
            regra(
                "X",
                r"C:\Outro\x.exe",
                Direcao::Entrada,
                Acao::Bloquear,
                0x7FFF_FFFF,
                true,
            ),
        ]);
        let plano = planejar_correcao(Path::new(EXE), &c);
        assert_eq!(plano.remover, vec!["A".to_string(), "B".to_string()]);
        assert!(plano.liberar_perfis.is_empty());
    }

    #[test]
    fn planejar_libera_so_perfil_do_radmin() {
        let c = Coleta {
            categoria_radmin: Some("Publica".to_string()),
            perfis: [
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
            ],
            ..Coleta::default()
        };
        let plano = planejar_correcao(Path::new(EXE), &c);
        assert_eq!(plano.liberar_perfis, vec![2]);
    }

    #[test]
    fn planejar_sem_radmin_nao_libera_perfil() {
        let c = Coleta {
            categoria_radmin: None,
            perfis: [
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
                PerfilBruto {
                    ativo: true,
                    bloquear_tudo_entrada: true,
                },
            ],
            ..Coleta::default()
        };
        let plano = planejar_correcao(Path::new(EXE), &c);
        assert!(plano.liberar_perfis.is_empty());
    }
}
