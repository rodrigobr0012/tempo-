//! Tempo para Windows: a interface web (`tempo-web`) numa janela nativa (Tauri).
//!
//! A interface continua rodando o `tempo-store` em WebAssembly; este processo cuida do que o
//! navegador não faz bem: guardar o banco num arquivo SQLite no PC, avisar o fim do timer com
//! notificação do Windows mesmo com a janela fechada, ícone na bandeja e início com o Windows.
//! É o equivalente do `Bridge.java` do Android.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, io, thread};

use serde::Deserialize;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;

const TRAY_ID: &str = "main";
/// Argumento usado pelo início automático: abre direto na bandeja.
const MINIMIZED: &str = "--minimized";

#[derive(Default)]
struct Timer {
    /// Muda a cada sincronização; o aviso agendado só dispara se ainda for o mais recente.
    generation: AtomicU64,
}

#[derive(Default)]
struct Hints {
    tray_explained: AtomicBool,
}

/// Timer em andamento, enviado pela interface a cada mudança.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerSync {
    tag_name: String,
    /// Instante (ms Unix) em que o tempo da tag acaba.
    deadline_ms: i64,
    /// Texto do ícone da bandeja, ex.: "Estudo · termina às 09:01".
    tooltip: String,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Grava de forma atômica: arquivo temporário + troca, para nunca deixar o banco pela metade.
fn write_atomic(path: &PathBuf, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn raw_body<'a>(request: &'a Request<'_>) -> Result<&'a [u8], String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes),
        _ => Err("esperado corpo binário".into()),
    }
}

/// Bytes do banco (`tempo.db`); vazio na primeira execução.
#[tauri::command]
fn load_db(app: AppHandle) -> Result<Response, String> {
    let path = data_dir(&app)?.join("tempo.db");
    match fs::read(&path) {
        Ok(bytes) => Ok(Response::new(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Response::new(Vec::new())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[tauri::command]
fn save_db(app: AppHandle, request: Request<'_>) -> Result<(), String> {
    let path = data_dir(&app)?.join("tempo.db");
    write_atomic(&path, raw_body(&request)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// Guarda à parte um banco que não abriu, para não perder dados.
#[tauri::command]
fn backup_db(app: AppHandle, request: Request<'_>) -> Result<(), String> {
    let path = data_dir(&app)?.join(format!("tempo-backup-{}.db", now_ms()));
    fs::write(&path, raw_body(&request)?).map_err(|e| e.to_string())
}

/// Atualiza a bandeja e agenda (ou cancela) o aviso de fim do timer.
#[tauri::command]
fn sync_timer(app: AppHandle, timer: State<'_, Timer>, sync: Option<TimerSync>) {
    let generation = timer.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let text = sync.as_ref().map_or_else(|| "Tempo".to_string(), |s| format!("Tempo — {}", s.tooltip));
        let _ = tray.set_tooltip(Some(text));
    }
    let Some(sync) = sync else { return };
    if sync.deadline_ms <= now_ms() {
        return;
    }
    thread::spawn(move || {
        // Espera em passos curtos conferindo o relógio: assim o aviso sai certo mesmo se o PC
        // dormir no meio do caminho.
        loop {
            if app.state::<Timer>().generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let left = sync.deadline_ms - now_ms();
            if left <= 0 {
                break;
            }
            thread::sleep(Duration::from_millis(left.min(15_000) as u64));
        }
        let _ = app
            .notification()
            .builder()
            .title("Tempo esgotado")
            .body(format!("O tempo de \"{}\" acabou.", sync.tag_name))
            .show();
    });
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        // Abrir de novo (atalho, início automático) só traz a janela existente para a frente.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED]),
        ))
        .manage(Timer::default())
        .manage(Hints::default())
        .invoke_handler(tauri::generate_handler![load_db, save_db, backup_db, sync_timer])
        .setup(|app| {
            let open = MenuItem::with_id(app, "open", "Abrir o Tempo", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Tempo")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } =
                        event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            if !std::env::args().any(|a| a == MINIMIZED) {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fechar a janela só esconde: o timer e o aviso de fim continuam na bandeja.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                let app = window.app_handle();
                if !app.state::<Hints>().tray_explained.swap(true, Ordering::SeqCst) {
                    let _ = app
                        .notification()
                        .builder()
                        .title("O Tempo continua aberto")
                        .body("Ele fica no ícone perto do relógio. Para sair, clique nele com o botão direito.")
                        .show();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Tempo");
}
