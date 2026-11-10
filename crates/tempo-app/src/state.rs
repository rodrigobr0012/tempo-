//! Instância única do serviço, compartilhada entre a interface e os receptores Android
//! (botões da notificação, alarme, boot), que podem rodar em outra thread do mesmo processo.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tempo_core::SystemClock;
use tempo_store::Tempo;

static STATE: Mutex<Option<(PathBuf, Tempo)>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<(PathBuf, Tempo)>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Abre o banco em `path` (uma vez por processo).
pub fn init(path: &Path) -> tempo_store::Result<()> {
    let mut g = lock();
    if g.as_ref().is_some_and(|(p, _)| p == path) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tempo = Tempo::open(path, Arc::new(SystemClock))?;
    *g = Some((path.to_path_buf(), tempo));
    Ok(())
}

/// Executa `f` com acesso exclusivo ao serviço.
pub fn with<R>(f: impl FnOnce(&mut Tempo) -> tempo_store::Result<R>) -> tempo_store::Result<R> {
    let mut g = lock();
    match g.as_mut() {
        Some((_, t)) => f(t),
        None => Err(tempo_store::Error::NotOnboarded),
    }
}
