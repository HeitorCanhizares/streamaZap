//! Antivírus com firewall próprio (WMI `root/SecurityCenter2`, classe `FirewallProduct`).

/// Remove da lista o próprio Windows (nomes contendo "defender" ou "windows", sem diferenciar
/// maiúsculas) e vazios, sem repetição, ordem preservada. Função pura.
pub fn filtrar_terceiros(nomes: &[String]) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for nome in nomes {
        let nome = nome.trim();
        if nome.is_empty() {
            continue;
        }
        let minusculo = nome.to_lowercase();
        if minusculo.contains("defender") || minusculo.contains("windows") {
            continue;
        }
        if !saida.iter().any(|s| s == nome) {
            saida.push(nome.to_string());
        }
    }
    saida
}

/// Nomes (`displayName`) dos firewalls de terceiros. Windows: consulta WMI e filtra. Outros
/// sistemas ou erro: lista vazia.
pub fn firewalls_de_terceiros() -> Vec<String> {
    #[cfg(not(windows))]
    {
        Vec::new()
    }
    #[cfg(windows)]
    {
        match consultar_wmi() {
            Ok(nomes) => filtrar_terceiros(&nomes),
            Err(e) => {
                tracing::debug!("consulta WMI de firewalls falhou: {e}");
                Vec::new()
            }
        }
    }
}

#[cfg(windows)]
fn consultar_wmi() -> windows::core::Result<Vec<String>> {
    use windows::core::{w, BSTR, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoInitializeSecurity, CoSetProxyBlanket, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_AUTHN_LEVEL_DEFAULT,
        RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_BSTR};
    use windows::Win32::System::Wmi::{
        IEnumWbemClassObject, IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY,
        WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE,
    };

    // Tolera S_FALSE e RPC_E_CHANGED_MODE: o COM só precisa estar inicializado na thread.
    // SAFETY: chamada sem ponteiros; o resultado é ignorado de propósito.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    // Opcional: se a segurança do processo já foi definida, o erro é inofensivo.
    // SAFETY: todos os ponteiros opcionais são nulos/None, o que a API aceita.
    let _ = unsafe {
        CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        )
    };

    // SAFETY: CLSID válido; COM inicializado acima (ou falha tratada pelo `?`).
    let locator: IWbemLocator = unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)? };

    // SAFETY: BSTRs vivos durante a chamada; contexto ausente.
    let servico: IWbemServices = unsafe {
        locator.ConnectServer(
            &BSTR::from("ROOT\\SecurityCenter2"),
            &BSTR::new(),
            &BSTR::new(),
            &BSTR::new(),
            0,
            &BSTR::new(),
            None,
        )?
    };

    // SAFETY: `servico` é um proxy COM válido; autenticação padrão do WMI (WINNT = 10).
    unsafe {
        CoSetProxyBlanket(
            &servico,
            10,
            0,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
    }

    // SAFETY: strings BSTR vivas durante a chamada.
    let enumerador: IEnumWbemClassObject = unsafe {
        servico.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from("SELECT displayName FROM FirewallProduct"),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?
    };

    let mut nomes = Vec::new();
    loop {
        let mut objs: [Option<IWbemClassObject>; 1] = [None];
        let mut retornados: u32 = 0;
        // SAFETY: `objs` e `retornados` são locais válidos durante a chamada.
        let hr = unsafe { enumerador.Next(WBEM_INFINITE, &mut objs, &mut retornados) };
        if hr.is_err() || retornados == 0 {
            break;
        }
        let Some(obj) = objs[0].take() else { break };

        let mut variante = VARIANT::default();
        // SAFETY: `variante` é um VARIANT válido; o nome é uma string literal wide.
        let lido = unsafe { obj.Get(w!("displayName"), 0, &mut variante, None, None) };
        if lido.is_ok() {
            // SAFETY: só lemos `bstrVal` quando `vt` é VT_BSTR, o membro ativo da união.
            let nome = unsafe {
                let interno = &variante.Anonymous.Anonymous;
                if interno.vt == VT_BSTR {
                    Some(interno.Anonymous.bstrVal.to_string())
                } else {
                    None
                }
            };
            if let Some(n) = nome {
                nomes.push(n);
            }
        }
        // SAFETY: `variante` foi inicializado por `default`/`Get`; libera o BSTR interno.
        let _ = unsafe { VariantClear(&mut variante) };
    }
    Ok(nomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(itens: &[&str]) -> Vec<String> {
        itens.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn filtrar_remove_defender_e_windows() {
        let r = filtrar_terceiros(&v(&["Windows Defender", "Windows Firewall", "WINDOWS", "Kaspersky"]));
        assert_eq!(r, v(&["Kaspersky"]));
    }

    #[test]
    fn filtrar_remove_vazios_e_duplicados() {
        let r = filtrar_terceiros(&v(&["", "   ", "Norton", " Norton ", "Norton"]));
        assert_eq!(r, v(&["Norton"]));
    }

    #[test]
    fn filtrar_preserva_ordem_e_terceiros() {
        let r = filtrar_terceiros(&v(&["Kaspersky", "Windows Defender", "Norton Firewall"]));
        assert_eq!(r, v(&["Kaspersky", "Norton Firewall"]));
    }

    #[test]
    fn firewalls_de_terceiros_nao_entra_em_panico() {
        let _ = firewalls_de_terceiros();
    }
}
