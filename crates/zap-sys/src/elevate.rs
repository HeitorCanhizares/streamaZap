//! Elevação de privilégio (UAC) e arquivo temporário de resultado.

use std::path::PathBuf;

use crate::firewall::Resultado;
use crate::{ErroSys, Result};

/// Argumento de linha de comando que ativa o modo "corrigir firewall" do app.
pub const ARG_CORRIGIR_FIREWALL: &str = "--corrigir-firewall";

/// Caminho do arquivo JSON temporário com o `Resultado` (`%TEMP%/streamazap-firewall.json`).
pub fn caminho_resultado() -> PathBuf {
    std::env::temp_dir().join("streamazap-firewall.json")
}

/// Junta argumentos numa linha de comando do Windows (aspas e barras conforme as regras do
/// `CommandLineToArgvW`). Função pura.
pub fn montar_linha_comando(args: &[String]) -> String {
    args.iter().map(|a| citar_argumento(a)).collect::<Vec<_>>().join(" ")
}

/// Cita um argumento só quando precisa (vazio, espaço, tab ou aspas). Barras antes de aspas
/// são dobradas, senão o `CommandLineToArgvW` leria a aspa como fim do argumento.
fn citar_argumento(arg: &str) -> String {
    let precisa = arg.is_empty() || arg.chars().any(|c| matches!(c, ' ' | '\t' | '"'));
    if !precisa {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut barras = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => barras += 1,
            '"' => {
                // Barras antes de aspa são dobradas e a aspa vira `\"`.
                out.push_str(&"\\".repeat(barras * 2 + 1));
                out.push('"');
                barras = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(barras));
                out.push(c);
                barras = 0;
            }
        }
    }
    // Barras no fim ficariam antes da aspa de fechamento, então também são dobradas.
    out.push_str(&"\\".repeat(barras * 2));
    out.push('"');
    out
}

/// Grava o resultado como JSON em `caminho_resultado()`.
pub fn escrever_resultado(r: &Resultado) -> Result<()> {
    let bytes = serde_json::to_vec(r).map_err(|e| ErroSys::Outro(e.to_string()))?;
    std::fs::write(caminho_resultado(), bytes).map_err(|e| ErroSys::Outro(e.to_string()))
}

/// Lê (e apaga) o resultado gravado; `None` se não existe ou é inválido.
pub fn ler_resultado() -> Option<Resultado> {
    let caminho = caminho_resultado();
    let bytes = std::fs::read(&caminho).ok()?;
    // Apaga mesmo se o conteúdo for inválido: não deixa lixo para a próxima execução.
    let _ = std::fs::remove_file(&caminho);
    // O PowerShell e alguns editores gravam BOM UTF-8; o serde_json não aceita.
    let corpo = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes.as_slice());
    serde_json::from_slice(corpo).ok()
}

/// O processo atual tem token elevado? Fora do Windows: `false`.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    // SAFETY: `GetCurrentProcess` devolve um pseudo-handle válido; `token` é um destino válido.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut elevacao = TOKEN_ELEVATION::default();
    let mut tamanho = 0u32;
    // SAFETY: o buffer aponta para `elevacao`, que tem exatamente o tamanho informado.
    let r = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevacao as *mut TOKEN_ELEVATION as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut tamanho,
        )
    };
    // SAFETY: `token` foi aberto acima e ainda não foi fechado.
    let _ = unsafe { CloseHandle(token) };
    r.is_ok() && elevacao.TokenIsElevated != 0
}

/// O processo atual tem token elevado? Fora do Windows: `false`.
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Relança o executável atual elevado (`ShellExecuteExW` verbo `runas`, janela oculta), espera
/// terminar e devolve o código de saída. Usuário cancelou o UAC -> `Err(ErroSys::SemPermissao)`.
/// Fora do Windows: `Err(NaoSuportado)`.
#[cfg(windows)]
pub fn relaunch_elevated(args: &[String]) -> Result<u32> {
    use windows::core::{w, HRESULT, PCWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    // HRESULT de ERROR_CANCELLED (1223): o usuário recusou o UAC.
    const CANCELADO: HRESULT = HRESULT(0x800704C7u32 as i32);

    let exe = std::env::current_exe().map_err(|e| ErroSys::Outro(e.to_string()))?;
    let arquivo: Vec<u16> = exe.as_os_str().encode_wide_nul();
    let parametros: Vec<u16> = montar_linha_comando(args).encode_utf16().chain(Some(0)).collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(arquivo.as_ptr()),
        lpParameters: PCWSTR(parametros.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` está inicializado e os ponteiros (`arquivo`, `parametros`) vivem até o fim.
    if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
        return Err(if e.code() == CANCELADO {
            ErroSys::SemPermissao
        } else {
            ErroSys::Outro(e.to_string())
        });
    }
    let processo = info.hProcess;
    if processo.is_invalid() {
        return Err(ErroSys::Outro("processo elevado não devolveu handle".to_string()));
    }
    let mut codigo = 0u32;
    // SAFETY: `processo` é um handle válido devolvido por `ShellExecuteExW` (NOCLOSEPROCESS) e
    // é fechado aqui, uma única vez.
    let r = unsafe {
        WaitForSingleObject(processo, INFINITE);
        let r = GetExitCodeProcess(processo, &mut codigo);
        let _ = CloseHandle(processo);
        r
    };
    r.map_err(|e| ErroSys::Outro(e.to_string()))?;
    Ok(codigo)
}

/// Relança o executável atual elevado. Fora do Windows: `Err(NaoSuportado)`.
#[cfg(not(windows))]
pub fn relaunch_elevated(_args: &[String]) -> Result<u32> {
    Err(ErroSys::NaoSuportado)
}

/// Converte um `OsStr` em UTF-16 terminado em nulo.
#[cfg(windows)]
trait CodificarUtf16 {
    fn encode_wide_nul(&self) -> Vec<u16>;
}

#[cfg(windows)]
impl CodificarUtf16 for std::ffi::OsStr {
    fn encode_wide_nul(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(Some(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    // O arquivo de resultado é um só por usuário: os testes que gravam/leem rodam em série.
    static TRAVA: Mutex<()> = Mutex::new(());

    fn trava() -> MutexGuard<'static, ()> {
        TRAVA.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn resultado_exemplo() -> Resultado {
        Resultado {
            ok: true,
            mensagem: "regras removidas".to_string(),
            removidas: 3,
            perfis_liberados: vec!["Público".to_string()],
        }
    }

    #[test]
    fn linha_comando_simples() {
        assert_eq!(montar_linha_comando(&sv(&["--a", "b"])), "--a b");
    }

    #[test]
    fn linha_comando_com_espaco() {
        assert_eq!(montar_linha_comando(&sv(&["--a", "b c"])), "--a \"b c\"");
    }

    #[test]
    fn linha_comando_vazio() {
        assert_eq!(montar_linha_comando(&sv(&[""])), "\"\"");
    }

    #[test]
    fn linha_comando_barra_final_dobrada() {
        // `C:\x y\` -> a barra final vira duas, antes da aspa de fechamento.
        assert_eq!(montar_linha_comando(&sv(&["C:\\x y\\"])), "\"C:\\x y\\\\\"");
    }

    #[test]
    fn linha_comando_aspas_internas() {
        // `a"b` -> `"a\"b"`.
        assert_eq!(montar_linha_comando(&sv(&["a\"b"])), "\"a\\\"b\"");
    }

    #[test]
    fn resultado_ida_e_volta() {
        let _g = trava();
        let r = resultado_exemplo();
        assert!(escrever_resultado(&r).is_ok());
        assert_eq!(ler_resultado(), Some(r));
        assert!(!caminho_resultado().exists());
    }

    #[test]
    fn ler_sem_arquivo_e_none() {
        let _g = trava();
        let _ = std::fs::remove_file(caminho_resultado());
        assert_eq!(ler_resultado(), None);
    }

    #[test]
    fn ler_json_invalido_e_none() {
        let _g = trava();
        assert!(std::fs::write(caminho_resultado(), b"{ invalido").is_ok());
        assert_eq!(ler_resultado(), None);
        assert!(!caminho_resultado().exists());
    }

    #[test]
    fn ler_com_bom() {
        let _g = trava();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(br#"{"ok":false,"mensagem":"oi","removidas":1,"perfis_liberados":[]}"#);
        assert!(std::fs::write(caminho_resultado(), bytes).is_ok());
        assert_eq!(
            ler_resultado(),
            Some(Resultado {
                ok: false,
                mensagem: "oi".to_string(),
                removidas: 1,
                perfis_liberados: vec![],
            })
        );
    }

    #[test]
    fn is_elevated_nao_entra_em_panico() {
        let _ = is_elevated();
    }

    #[cfg(not(windows))]
    #[test]
    fn relaunch_fora_do_windows_erra_nao_suportado() {
        assert!(matches!(relaunch_elevated(&[]), Err(ErroSys::NaoSuportado)));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "abre o UAC"]
    fn relaunch_abre_o_uac() {
        let _ = relaunch_elevated(&sv(&["--versao"]));
    }
}
