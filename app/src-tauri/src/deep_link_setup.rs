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

/// `true` se o esquema precisa ser registrado: o `is_registered` não confirmou que ele já abre
/// este executável (não registrado, outro exe ou erro ao ler).
#[cfg(any(debug_assertions, windows))]
fn precisa_registrar<E>(registrado_aqui: Result<bool, E>) -> bool {
    !matches!(registrado_aqui, Ok(true))
}

/// Faz o `streamazap://` abrir este executável, se ainda não abre. O instalador NSIS já registra
/// o esquema, mas o portátil (zip) e o app em dev não passam por ele; sem isso, o convite e a
/// volta do login não abrem o app. Vale o último app aberto (instalado, portátil ou dev). No
/// Windows é a mesma chave que o NSIS usa (`HKCU\Software\Classes\streamazap`, instalação
/// `currentUser`), e o desinstalador a apaga. No Linux, só em desenvolvimento.
#[cfg(any(debug_assertions, windows))]
fn registrar_esquema(app: &tauri::App) {
    let dl = app.deep_link();
    if precisa_registrar(dl.is_registered(crate::deep_link::SCHEME)) && dl.register_all().is_err() {
        // Sem detalhes: o erro pode conter caminhos/URLs.
        tracing::warn!("não foi possível registrar o esquema streamazap://");
    }
}

/// Liga o plugin: `on_open_url` (instância aberta, inclusive via single-instance) emite
/// [`crate::deep_link::EVENT`] para cada URL de callback e [`crate::deep_link::EVENT_INVITE`] para
/// cada convite de sala; a URL que abriu o app (`get_current`) vai para [`DeepLinkPendentes`] ou
/// [`ConvitesPendentes`], porque a interface ainda não está escutando. Antes, registra o esquema
/// para este executável ([`registrar_esquema`]: no Windows sempre, no Linux só em dev).
pub fn setup(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    #[cfg(any(debug_assertions, windows))]
    registrar_esquema(app);

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
    fn registra_so_se_nao_abre_este_exe() {
        assert!(!precisa_registrar::<()>(Ok(true)));
        assert!(precisa_registrar::<()>(Ok(false)));
        assert!(precisa_registrar(Err("leitura do registro falhou")));
    }

    #[test]
    fn plugin_registrado_e_setup_chamado() {
        assert!(include_str!("lib.rs").contains("tauri_plugin_deep_link::init()"));
        assert!(include_str!("setup.rs").contains("deep_link_setup::setup"));
    }
}
