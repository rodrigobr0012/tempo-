//! App de gestão de tempo por disponibilidade — interface Slint e integração Android.

slint::include_modules!();

pub mod platform;
mod pour;
mod present;
pub mod state;
mod ui;

#[cfg(target_os = "android")]
mod android;

#[cfg(not(target_os = "android"))]
pub mod devtools;

use std::path::PathBuf;
use std::rc::Rc;

use platform::Platform;

/// Abre o banco, monta a interface e roda o laço de eventos.
pub fn run(db_path: PathBuf, platform: Rc<dyn Platform>) -> Result<(), slint::PlatformError> {
    if let Err(e) = state::init(&db_path) {
        log::error!("não foi possível abrir {}: {e}", db_path.display());
    }
    let window = AppWindow::new()?;
    let _handle = ui::setup(&window, platform);
    window.run()
}
