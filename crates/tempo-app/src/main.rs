//! Executável de desenvolvimento no desktop. O app de verdade roda no Android.

#[cfg(not(target_os = "android"))]
fn main() -> Result<(), slint::PlatformError> {
    use std::path::PathBuf;
    use std::rc::Rc;

    if let Some(dir) = std::env::var_os("TEMPO_SNAPSHOTS") {
        return tempo_app::devtools::snapshots(PathBuf::from(dir));
    }
    let db = std::env::var_os("TEMPO_DB").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("tempo-desktop.db"));
    let data_dir = db.parent().filter(|p| !p.as_os_str().is_empty()).map(PathBuf::from).unwrap_or_else(|| ".".into());
    tempo_app::run(db, Rc::new(tempo_app::platform::DesktopPlatform { data_dir }))
}

#[cfg(target_os = "android")]
fn main() {}
