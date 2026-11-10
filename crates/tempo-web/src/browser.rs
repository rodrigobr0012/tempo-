//! Ponte com as APIs do navegador (implementadas em `browser.js`).

use js_sys::{Promise, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/browser.js")]
extern "C" {
    #[wasm_bindgen(js_name = loadDb)]
    fn load_db_js() -> Promise;
    #[wasm_bindgen(js_name = saveDb)]
    fn save_db_js(bytes: &[u8]) -> Promise;
    #[wasm_bindgen(js_name = backupDb)]
    fn backup_db_js(bytes: &[u8]) -> Promise;
    #[wasm_bindgen(js_name = onOtherTabSaved)]
    fn on_other_tab_saved_js(cb: &js_sys::Function);
    #[wasm_bindgen(js_name = persistStorage)]
    pub fn persist_storage();
    #[wasm_bindgen(js_name = notificationPermission)]
    fn notification_permission_js() -> String;
    #[wasm_bindgen(js_name = requestNotifications)]
    fn request_notifications_js() -> Promise;
    pub fn notify(title: &str, body: &str);
    #[wasm_bindgen(js_name = unlockAudio)]
    pub fn unlock_audio();
    pub fn beep();
    #[wasm_bindgen(js_name = canVibrate)]
    pub fn can_vibrate() -> bool;
    pub fn vibrate();
    pub fn download(name: &str, text: &str, mime: &str);
    #[wasm_bindgen(js_name = isDesktop)]
    pub fn is_desktop() -> bool;
    #[wasm_bindgen(js_name = syncTimer)]
    fn sync_timer_js(tag_name: &str, deadline_ms: f64, tooltip: &str);
    #[wasm_bindgen(js_name = autostartEnabled)]
    fn autostart_enabled_js() -> Promise;
    #[wasm_bindgen(js_name = setAutostart)]
    fn set_autostart_js(on: bool) -> Promise;
}

/// Programa para Windows: atualiza a bandeja e agenda o aviso de fim (`None` = sem timer).
pub fn sync_timer(timer: Option<(&str, i64, &str)>) {
    match timer {
        Some((name, deadline_ms, tooltip)) => sync_timer_js(name, deadline_ms as f64, tooltip),
        None => sync_timer_js("", 0.0, ""),
    }
}

pub async fn autostart_enabled() -> bool {
    JsFuture::from(autostart_enabled_js()).await.ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

pub async fn set_autostart(on: bool) -> Result<(), String> {
    JsFuture::from(set_autostart_js(on)).await.map(|_| ()).map_err(js_err)
}

/// Bytes do banco salvo no IndexedDB (`None` = primeira vez neste navegador).
pub async fn load_db() -> Result<Option<Vec<u8>>, String> {
    let v = JsFuture::from(load_db_js()).await.map_err(js_err)?;
    if v.is_null() || v.is_undefined() {
        return Ok(None);
    }
    Ok(Some(Uint8Array::new(&v).to_vec()))
}

pub async fn save_db(bytes: &[u8]) -> Result<(), String> {
    // A cópia acontece de forma síncrona dentro de `saveDb`, antes de `bytes` sair de escopo.
    let p = save_db_js(bytes);
    JsFuture::from(p).await.map(|_| ()).map_err(js_err)
}

pub async fn backup_db(bytes: &[u8]) -> Result<(), String> {
    let p = backup_db_js(bytes);
    JsFuture::from(p).await.map(|_| ()).map_err(js_err)
}

pub fn on_other_tab_saved(f: impl FnMut() + 'static) {
    let cb = Closure::<dyn FnMut()>::new(f);
    on_other_tab_saved_js(cb.as_ref().unchecked_ref());
    cb.forget();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Granted,
    Denied,
    Default,
    Unsupported,
}

fn parse_permission(s: &str) -> Permission {
    match s {
        "granted" => Permission::Granted,
        "denied" => Permission::Denied,
        "default" => Permission::Default,
        _ => Permission::Unsupported,
    }
}

pub fn notification_permission() -> Permission {
    parse_permission(&notification_permission_js())
}

pub async fn request_notifications() -> Permission {
    match JsFuture::from(request_notifications_js()).await {
        Ok(v) => parse_permission(&v.as_string().unwrap_or_default()),
        Err(_) => notification_permission(),
    }
}

fn js_err(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| format!("{e:?}"))
}
