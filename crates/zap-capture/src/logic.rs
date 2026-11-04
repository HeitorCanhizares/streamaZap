//! Lógica pura (sem Windows) usada pelas capturas; testável em qualquer sistema.
use crate::MonitorInfo;
use std::time::Duration;

/// Dados crus de uma janela, já lidos do Windows, para decidir se ela entra na lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCandidate {
    pub visible: bool,
    pub cloaked: bool,
    pub tool_window: bool,
    pub title: String,
    pub pid: u32,
    pub width: u32,
    pub height: u32,
}

/// `true` se a janela entra em `list_windows`.
pub fn keep_window(c: &WindowCandidate, own_pid: u32) -> bool {
    c.visible
        && !c.cloaked
        && !c.tool_window
        && !c.title.trim().is_empty()
        && c.pid != own_pid
        && c.width > 0
        && c.height > 0
}

/// Ordena: principal primeiro, depois por `x`, depois por `y`; renumera `index` de 1 em diante.
pub fn order_monitors(monitors: &mut [MonitorInfo]) {
    // `sort_by_key` é estável; `!primary` faz o principal (false) vir antes dos demais.
    monitors.sort_by_key(|m| (!m.primary, m.x, m.y));
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i as u32 + 1;
    }
}

/// Posição do cursor dentro da textura do monitor (origem no canto do monitor), já descontado o
/// hotspot. `None` se o ícone ficar totalmente fora do monitor.
/// `cursor` e `hotspot` em pixels de tela; `monitor` = (x, y, largura, altura); `icon` = (largura, altura).
pub fn cursor_local_pos(
    cursor: (i32, i32),
    hotspot: (i32, i32),
    icon: (u32, u32),
    monitor: (i32, i32, u32, u32),
) -> Option<(i32, i32)> {
    // Aritmética em i64 para não estourar com coordenadas extremas.
    let px = i64::from(cursor.0) - i64::from(hotspot.0) - i64::from(monitor.0);
    let py = i64::from(cursor.1) - i64::from(hotspot.1) - i64::from(monitor.1);
    let (iw, ih) = (i64::from(icon.0), i64::from(icon.1));
    let (mw, mh) = (i64::from(monitor.2), i64::from(monitor.3));

    // Retângulo do ícone [px, px+iw) x [py, py+ih) deve intersectar [0, mw) x [0, mh).
    let intersecta = px < mw && px + iw > 0 && py < mh && py + ih > 0;
    if !intersecta {
        return None;
    }
    // Fora de i32 só acontece com coordenadas absurdas; tratamos como fora da tela.
    Some((i32::try_from(px).ok()?, i32::try_from(py).ok()?))
}

/// Decide quando reentregar o último quadro (mantém o espectador vivo).
#[derive(Debug, Clone)]
pub struct StaleTimer {
    interval: Duration,
    last_us: Option<u64>,
}

impl StaleTimer {
    /// `interval` normalmente 1 s.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_us: None,
        }
    }
    /// Registra que um quadro (novo ou reentregue) saiu em `now_us`.
    pub fn delivered(&mut self, now_us: u64) {
        self.last_us = Some(now_us);
    }
    /// `true` se já passou `interval` desde o último quadro entregue. Antes do primeiro quadro: `false`.
    pub fn due(&self, now_us: u64) -> bool {
        match self.last_us {
            None => false,
            // u128 evita truncar `as_micros()` (que é u128) para u64.
            Some(last) => u128::from(now_us.saturating_sub(last)) >= self.interval.as_micros(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn janela_ok() -> WindowCandidate {
        WindowCandidate {
            visible: true,
            cloaked: false,
            tool_window: false,
            title: "Bloco de Notas".to_string(),
            pid: 100,
            width: 800,
            height: 600,
        }
    }

    fn mon(index: u32, name: &str, x: i32, y: i32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            id: index as isize,
            index,
            name: name.to_string(),
            x,
            y,
            width: 1920,
            height: 1080,
            primary,
            adapter_luid: 0,
        }
    }

    #[test]
    fn keep_window_aceita_normal() {
        assert!(keep_window(&janela_ok(), 999));
    }

    #[test]
    fn keep_window_rejeita_cada_condicao() {
        let own_pid = 42;

        let mut c = janela_ok();
        c.visible = false;
        assert!(!keep_window(&c, own_pid), "invisível");

        let mut c = janela_ok();
        c.cloaked = true;
        assert!(!keep_window(&c, own_pid), "cloaked");

        let mut c = janela_ok();
        c.tool_window = true;
        assert!(!keep_window(&c, own_pid), "tool_window");

        let mut c = janela_ok();
        c.title = String::new();
        assert!(!keep_window(&c, own_pid), "título vazio");

        let mut c = janela_ok();
        c.title = "   \t ".to_string();
        assert!(!keep_window(&c, own_pid), "título só com espaços");

        let mut c = janela_ok();
        c.pid = own_pid;
        assert!(!keep_window(&c, own_pid), "pid próprio");

        let mut c = janela_ok();
        c.width = 0;
        assert!(!keep_window(&c, own_pid), "largura zero");

        let mut c = janela_ok();
        c.height = 0;
        assert!(!keep_window(&c, own_pid), "altura zero");
    }

    #[test]
    fn order_monitors_principal_primeiro_e_renumera() {
        let mut ms = vec![
            mon(9, "C", 1920, 0, false),
            mon(9, "B", 0, 1080, false),
            mon(9, "P", 0, 0, true),
        ];
        order_monitors(&mut ms);

        let nomes: Vec<&str> = ms.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(nomes, vec!["P", "B", "C"]);

        let indices: Vec<u32> = ms.iter().map(|m| m.index).collect();
        assert_eq!(indices, vec![1, 2, 3]);
    }

    #[test]
    fn cursor_dentro_com_hotspot() {
        // cursor (100,100) - hotspot (4,4) = (96,96), monitor na origem.
        let p = cursor_local_pos((100, 100), (4, 4), (32, 32), (0, 0, 1920, 1080));
        assert_eq!(p, Some((96, 96)));
    }

    #[test]
    fn cursor_totalmente_fora_none() {
        // Ícone em (-100,-100) com tamanho 32: nada cai dentro de [0,1920)x[0,1080).
        let p = cursor_local_pos((-100, -100), (0, 0), (32, 32), (0, 0, 1920, 1080));
        assert_eq!(p, None);
        // Logo à direita do monitor, sem tocar a borda.
        let p = cursor_local_pos((1920, 10), (0, 0), (32, 32), (0, 0, 1920, 1080));
        assert_eq!(p, None);
    }

    #[test]
    fn cursor_parcialmente_fora_negativo() {
        // (5,5) - (10,10) = (-5,-5): ícone de 32 px ainda cobre (0..27, 0..27).
        let p = cursor_local_pos((5, 5), (10, 10), (32, 32), (0, 0, 1920, 1080));
        assert_eq!(p, Some((-5, -5)));
    }

    #[test]
    fn cursor_em_monitor_com_origem_negativa() {
        // Monitor à esquerda do principal: origem (-1920, 0).
        // (-1900,50) - (0,0) - (-1920,0) = (20,50).
        let p = cursor_local_pos((-1900, 50), (0, 0), (32, 32), (-1920, 0, 1920, 1080));
        assert_eq!(p, Some((20, 50)));
    }

    #[test]
    fn stale_timer_antes_do_primeiro_false() {
        let t = StaleTimer::new(Duration::from_secs(1));
        assert!(!t.due(0));
        assert!(!t.due(u64::MAX));
    }

    #[test]
    fn stale_timer_due_apos_intervalo() {
        let mut t = StaleTimer::new(Duration::from_secs(1));
        t.delivered(0);
        assert!(!t.due(999_999));
        assert!(t.due(1_000_000));

        // Reentregar reinicia a contagem.
        t.delivered(5_000_000);
        assert!(!t.due(5_500_000));
        assert!(t.due(6_000_000));
    }
}
