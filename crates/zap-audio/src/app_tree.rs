//! Árvore de processos e montagem da lista de apps com som (portável, sem chamar o sistema).
//!
//! O Chrome, o Edge, o Discord e o Spotify tocam o som num processo filho com o mesmo
//! executável. A lista mostra um item por app (a raiz da árvore) e a captura usa a árvore toda,
//! então o som continua vindo mesmo se o processo filho que toca for trocado.

use std::collections::HashMap;

use crate::{AppSound, AudioApp};

/// pid → (pid do pai, executável). No Windows vem de `apps::process_table`.
pub type ProcTable = HashMap<u32, (u32, String)>;

/// Limite ao subir na árvore: pids reaproveitados podem formar ciclos.
const MAX_PROFUNDIDADE: usize = 64;

/// Processos do próprio Windows com janela que nunca são o som que alguém quer mandar.
const RUIDO_DO_WINDOWS: &[&str] = &[
    "explorer.exe",
    "applicationframehost.exe",
    "textinputhost.exe",
    "systemsettings.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "searchapp.exe",
    "lockapp.exe",
];

fn mesmo_exe(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Sobe enquanto o pai tem o mesmo executável. Pid fora da tabela volta ele mesmo.
pub fn root_of(pid: u32, procs: &ProcTable) -> u32 {
    let Some((_, exe)) = procs.get(&pid) else {
        return pid;
    };
    let mut atual = pid;
    for _ in 0..MAX_PROFUNDIDADE {
        let Some((pai, _)) = procs.get(&atual) else {
            break;
        };
        let pai = *pai;
        if pai == 0 || pai == atual {
            break;
        }
        match procs.get(&pai) {
            Some((_, exe_pai)) if mesmo_exe(exe_pai, exe) => atual = pai,
            _ => break,
        }
    }
    atual
}

/// `pid` é `ancestral` ou descende dele (qualquer executável no caminho).
pub fn descends_from(pid: u32, ancestral: u32, procs: &ProcTable) -> bool {
    let mut atual = pid;
    for _ in 0..MAX_PROFUNDIDADE {
        if atual == ancestral {
            return true;
        }
        match procs.get(&atual) {
            Some((pai, _)) if *pai != 0 && *pai != atual => atual = *pai,
            _ => return false,
        }
    }
    false
}

/// Raiz de um app pelo executável (o app foi reaberto e ganhou outro pid). Entre várias, a de
/// menor pid; nunca da árvore do próprio processo.
pub fn root_by_exe(exe: &str, procs: &ProcTable, own_pid: u32) -> Option<u32> {
    let mut raizes: Vec<u32> = procs
        .iter()
        .filter(|(_, (_, e))| mesmo_exe(e, exe))
        .map(|(pid, _)| root_of(*pid, procs))
        .filter(|r| !descends_from(*r, own_pid, procs))
        .collect();
    raizes.sort_unstable();
    raizes.first().copied()
}

/// Agrupa os processos achados (com sessão de som ou só com janela) pela raiz do app. O melhor
/// estado de som vence (`Playing` antes de `Ready` antes de `Silent`). Ficam de fora a árvore do
/// próprio processo, pids que não estão na tabela (já fecharam) e, quando só têm janela, os
/// processos do próprio Windows. Devolve (raiz, executável, som).
pub fn group(found: &[(u32, AppSound)], procs: &ProcTable, own_pid: u32) -> Vec<(u32, String, AppSound)> {
    let mut por_raiz: HashMap<u32, (String, AppSound)> = HashMap::new();
    for &(pid, som) in found {
        if pid == 0 || !procs.contains_key(&pid) || descends_from(pid, own_pid, procs) {
            continue;
        }
        let raiz = root_of(pid, procs);
        let Some((_, exe)) = procs.get(&raiz) else {
            continue;
        };
        por_raiz
            .entry(raiz)
            .and_modify(|(_, s)| *s = (*s).min(som))
            .or_insert_with(|| (exe.clone(), som));
    }
    por_raiz
        .into_iter()
        .filter(|(_, (exe, som))| !(*som == AppSound::Silent && RUIDO_DO_WINDOWS.iter().any(|r| mesmo_exe(r, exe))))
        .map(|(pid, (exe, som))| (pid, exe, som))
        .collect()
}

/// Nome para mostrar: a descrição do executável ("Google Chrome") ou o nome sem ".exe".
pub fn display_name(description: Option<&str>, exe: &str) -> String {
    if let Some(d) = description.map(str::trim).filter(|d| !d.is_empty()) {
        return d.to_string();
    }
    let n = exe.len();
    if n > 4 && exe.is_char_boundary(n - 4) && exe[n - 4..].eq_ignore_ascii_case(".exe") {
        exe[..n - 4].to_string()
    } else {
        exe.to_string()
    }
}

/// Ordena (tocando, depois com som, depois só com janela; dentro, por nome) e põe o pid no
/// nome quando dois apps diferentes têm o mesmo nome (duas cópias de um jogo, por exemplo).
pub fn finish(apps: &mut [AudioApp]) {
    apps.sort_by(|a, b| {
        a.sound
            .cmp(&b.sound)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.pid.cmp(&b.pid))
    });
    let mut contagem: HashMap<String, usize> = HashMap::new();
    for a in apps.iter() {
        *contagem.entry(a.name.to_lowercase()).or_default() += 1;
    }
    for a in apps.iter_mut() {
        if contagem.get(&a.name.to_lowercase()).copied().unwrap_or(0) > 1 {
            a.name = format!("{} ({})", a.name, a.pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explorer(10) → chrome(100) → chrome(101, som) e chrome(102); streamazap(50) → webview(51).
    fn tabela() -> ProcTable {
        let mut t = ProcTable::new();
        t.insert(4, (0, "System".into()));
        t.insert(10, (4, "explorer.exe".into()));
        t.insert(100, (10, "chrome.exe".into()));
        t.insert(101, (100, "chrome.exe".into()));
        t.insert(102, (101, "Chrome.exe".into()));
        t.insert(50, (10, "streamazap.exe".into()));
        t.insert(51, (50, "msedgewebview2.exe".into()));
        t.insert(52, (51, "msedgewebview2.exe".into()));
        t.insert(200, (10, "Spotify.exe".into()));
        t.insert(300, (10, "jogo.exe".into()));
        t.insert(301, (10, "jogo.exe".into()));
        t
    }

    #[test]
    fn raiz_sobe_so_pelo_mesmo_executavel() {
        let t = tabela();
        assert_eq!(root_of(102, &t), 100);
        assert_eq!(root_of(101, &t), 100);
        assert_eq!(root_of(100, &t), 100);
        assert_eq!(root_of(52, &t), 51, "o pai streamazap.exe é outro executável");
        assert_eq!(root_of(999, &t), 999, "fora da tabela");
    }

    #[test]
    fn raiz_com_ciclo_nao_trava() {
        let mut t = ProcTable::new();
        t.insert(1, (2, "a.exe".into()));
        t.insert(2, (1, "a.exe".into()));
        let r = root_of(1, &t);
        assert!(r == 1 || r == 2);
        assert!(!descends_from(1, 99, &t));
    }

    #[test]
    fn descende_de_qualquer_executavel() {
        let t = tabela();
        assert!(descends_from(52, 50, &t));
        assert!(descends_from(50, 50, &t));
        assert!(!descends_from(101, 50, &t));
    }

    #[test]
    fn agrupa_pela_raiz_com_o_melhor_som() {
        let t = tabela();
        let achados = [
            (101, AppSound::Ready),
            (102, AppSound::Playing),
            (100, AppSound::Silent),
            (52, AppSound::Playing),  // árvore do próprio app: fora
            (999, AppSound::Playing), // já fechou: fora
            (10, AppSound::Silent),   // explorer só com janela: fora
            (200, AppSound::Silent),
            (0, AppSound::Playing),
        ];
        let mut g = group(&achados, &t, 50);
        g.sort();
        assert_eq!(
            g,
            vec![
                (100, "chrome.exe".to_string(), AppSound::Playing),
                (200, "Spotify.exe".to_string(), AppSound::Silent),
            ]
        );
    }

    #[test]
    fn explorer_com_sessao_de_som_aparece() {
        let t = tabela();
        let g = group(&[(10, AppSound::Ready)], &t, 50);
        assert_eq!(g, vec![(10, "explorer.exe".to_string(), AppSound::Ready)]);
    }

    #[test]
    fn raiz_pelo_executavel() {
        let t = tabela();
        assert_eq!(root_by_exe("CHROME.EXE", &t, 50), Some(100));
        assert_eq!(root_by_exe("jogo.exe", &t, 50), Some(300));
        assert_eq!(root_by_exe("msedgewebview2.exe", &t, 50), None, "árvore própria");
        assert_eq!(root_by_exe("nada.exe", &t, 50), None);
    }

    #[test]
    fn nome_para_mostrar() {
        assert_eq!(display_name(Some("  Google Chrome "), "chrome.exe"), "Google Chrome");
        assert_eq!(display_name(Some("   "), "cs2.exe"), "cs2");
        assert_eq!(display_name(None, "Spotify.EXE"), "Spotify");
        assert_eq!(display_name(None, "app"), "app");
        assert_eq!(display_name(None, ".exe"), ".exe");
    }

    fn app(pid: u32, name: &str, sound: AppSound) -> AudioApp {
        AudioApp {
            pid,
            name: name.into(),
            exe: format!("{name}.exe"),
            sound,
        }
    }

    #[test]
    fn ordena_e_desempata_nomes_iguais() {
        let mut v = vec![
            app(5, "zeta", AppSound::Silent),
            app(4, "Jogo", AppSound::Ready),
            app(3, "beta", AppSound::Playing),
            app(2, "jogo", AppSound::Silent),
            app(1, "Alfa", AppSound::Ready),
        ];
        finish(&mut v);
        let nomes: Vec<&str> = v.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(nomes, vec!["beta", "Alfa", "Jogo (4)", "jogo (2)", "zeta"]);
    }
}
