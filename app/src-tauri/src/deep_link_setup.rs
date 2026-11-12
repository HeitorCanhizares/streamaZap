//! Liga o plugin de deep link ao evento da interface (filtro puro em [`crate::deep_link`]).

use std::error::Error;

use parking_lot::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

/// Callbacks que chegaram antes de a interface escutar (o app aberto do zero pela URL do login).
/// A interface pega com o comando `deep_link_pendentes` quando liga o login.
#[derive(Default)]
pub struct DeepLinkPendentes(Mutex<Vec<String>>);

impl DeepLinkPendentes {
    /// Guarda os callbacks de `urls` (os outros são descartados).
    pub fn guardar(&self, urls: impl IntoIterator<Item = String>) {
        self.0.lock().extend(urls_para_emitir(urls));
    }

    /// Devolve e esvazia a fila (cada URL é entregue uma vez só).
    pub fn tirar(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock())
    }
}

/// Convites de sala que chegaram antes de a interface escutar (o app aberto do zero pelo link do
/// convite). Guarda os alvos já normalizados; a interface pega com `convites_pendentes`.
#[derive(Default)]
pub struct ConvitesPendentes(Mutex<Vec<String>>);

impl ConvitesPendentes {
    /// Guarda os alvos dos convites de `urls` (o resto é descartado).
    pub fn guardar(&self, urls: &[String]) {
        self.0.lock().extend(crate::deep_link::filter_invites(urls));
    }

    /// Devolve e esvazia a fila (cada convite é entregue uma vez só).
    pub fn tirar(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock())
    }
}

/// Filtra as URLs para só deixar os callbacks do OAuth, na ordem (parte pura, testável).
fn urls_para_emitir(urls: impl IntoIterator<Item = String>) -> Vec<String> {
    let todas: Vec<String> = urls.into_iter().collect();
    crate::deep_link::filter_callbacks(&todas)
}

/// Emite um evento por URL de callback e um por convite de sala. Nunca loga a URL do callback:
/// ela carrega o `code` do OAuth.
fn emitir(handle: &tauri::AppHandle, urls: Vec<String>) {
    for alvo in crate::deep_link::filter_invites(&urls) {
        if let Err(e) = handle.emit(crate::deep_link::EVENT_INVITE, alvo) {
            tracing::warn!("falha ao emitir o evento de convite: {e}");
        }
    }
    for url in urls_para_emitir(urls) {
        if let Err(e) = handle.emit(crate::deep_link::EVENT, url) {
            tracing::warn!("falha ao emitir o evento de deep link: {e}");
        }
    }
}

/// Liga o plugin: `on_open_url` (instância aberta, inclusive via single-instance) emite
/// [`crate::deep_link::EVENT`] para cada URL de callback e [`crate::deep_link::EVENT_INVITE`] para
/// cada convite de sala; a URL que abriu o app (`get_current`) vai para [`DeepLinkPendentes`] ou
/// [`ConvitesPendentes`], porque a interface ainda não está escutando. Em `debug_assertions`
/// chama `register_all()` (no instalado, o NSIS registra o esquema).
pub fn setup(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    #[cfg(debug_assertions)]
    if app.deep_link().register_all().is_err() {
        // Sem detalhes: o erro pode conter caminhos/URLs.
        tracing::warn!("não foi possível registrar o esquema streamazap:// em desenvolvimento");
    }

    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        emitir(&handle, event.urls().into_iter().map(|u| u.to_string()).collect());
    });

    // Primeira instância aberta pela URL: a interface ainda não existe, então a URL espera na
    // fila até o login ligar e chamar `deep_link_pendentes` (ou, no convite, `convites_pendentes`).
    let pendentes = DeepLinkPendentes::default();
    let convites = ConvitesPendentes::default();
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        let urls: Vec<String> = urls.into_iter().map(|u| u.to_string()).collect();
        convites.guardar(&urls);
        pendentes.guardar(urls);
    }
    app.manage(pendentes);
    app.manage(convites);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn so_callbacks_passam() {
        let r = urls_para_emitir(s(&[
            "streamazap://auth/callback?code=1",
            "https://x",
            "streamazap://join/abc",
        ]));
        assert_eq!(r, s(&["streamazap://auth/callback?code=1"]));
    }

    #[test]
    fn vazio_vazio() {
        assert!(urls_para_emitir(Vec::<String>::new()).is_empty());
    }

    #[test]
    fn ordem_preservada() {
        let r = urls_para_emitir(s(&[
            "streamazap://auth/callback?code=2",
            "https://x",
            "streamazap://auth/callback?code=1",
        ]));
        assert_eq!(
            r,
            s(&["streamazap://auth/callback?code=2", "streamazap://auth/callback?code=1"])
        );
    }

    #[test]
    fn pendentes_guarda_so_callbacks_e_entrega_uma_vez() {
        let p = DeepLinkPendentes::default();
        p.guardar(s(&["https://x", "streamazap://auth/callback?code=1"]));
        p.guardar(s(&["streamazap://auth/callback?code=2"]));
        assert_eq!(
            p.tirar(),
            s(&["streamazap://auth/callback?code=1", "streamazap://auth/callback?code=2"])
        );
        assert!(p.tirar().is_empty());
    }

    #[test]
    fn convites_guarda_so_convites_normalizados_e_entrega_uma_vez() {
        let c = ConvitesPendentes::default();
        c.guardar(&s(&["streamazap://auth/callback?code=1", "streamazap://sala/zap-7k2q"]));
        c.guardar(&s(&["streamazap://sala/26.14.7.88"]));
        assert_eq!(c.tirar(), s(&["ZAP-7K2Q", "26.14.7.88:47800"]));
        assert!(c.tirar().is_empty());
    }

    #[test]
    fn convite_nao_vira_callback() {
        assert!(urls_para_emitir(s(&["streamazap://sala/ZAP-7K2Q"])).is_empty());
    }

    #[test]
    fn plugin_registrado_e_setup_chamado() {
        assert!(include_str!("lib.rs").contains("tauri_plugin_deep_link::init()"));
        assert!(include_str!("setup.rs").contains("deep_link_setup::setup"));
    }
}
