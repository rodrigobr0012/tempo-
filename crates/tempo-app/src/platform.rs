//! Serviços do sistema operacional usados pela interface (notificação, alarme, permissões,
//! compartilhamento). No Android são implementados via JNI em `android.rs`; no desktop,
//! uma versão simples permite desenvolver e testar a interface.

use std::path::PathBuf;

use tempo_store::Tempo;

/// Estado do timer enviado ao sistema para a notificação persistente e o alarme de fim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimerSync {
    pub tag_name: String,
    pub color: u32,
    pub start_ms: i64,
    pub deadline_ms: i64,
    pub overtime: bool,
    pub sound: bool,
    pub vibration: bool,
}

/// Calcula o que o sistema deve mostrar agora (ou `None` para limpar).
pub fn timer_sync(t: &Tempo) -> tempo_store::Result<Option<TimerSync>> {
    let Some(a) = t.active_timer()? else { return Ok(None) };
    let s = t.settings()?;
    Ok(Some(TimerSync {
        tag_name: a.tag_name,
        color: a.color,
        start_ms: a.start * 1000,
        deadline_ms: a.deadline * 1000,
        overtime: s.overtime,
        sound: s.sound,
        vibration: s.vibration,
    }))
}

pub trait Platform {
    fn is_android(&self) -> bool;
    /// Mostra/atualiza a notificação do timer e agenda o alarme de fim, ou limpa ambos.
    fn sync_timer(&self, timer: Option<&TimerSync>);
    /// Aviso de "tempo esgotado" (RF20) quando o timer é encerrado com o app aberto.
    fn notify_finished(&self, tag_name: &str, sound: bool, vibration: bool);
    fn notifications_allowed(&self) -> bool;
    fn request_notifications(&self);
    fn exact_alarms_allowed(&self) -> bool;
    fn request_exact_alarms(&self);
    /// Compartilha um texto (ex.: CSV). Retorna a mensagem a exibir ao usuário.
    fn share_text(&self, title: &str, text: &str) -> Result<String, String>;
}

/// Implementação para desenvolvimento no desktop.
pub struct DesktopPlatform {
    pub data_dir: PathBuf,
}

impl Platform for DesktopPlatform {
    fn is_android(&self) -> bool {
        false
    }

    fn sync_timer(&self, timer: Option<&TimerSync>) {
        match timer {
            Some(t) => log::info!("timer: {} até {} (excedente: {})", t.tag_name, t.deadline_ms, t.overtime),
            None => log::info!("timer: nenhum"),
        }
    }

    fn notify_finished(&self, tag_name: &str, _sound: bool, _vibration: bool) {
        log::info!("tempo esgotado: {tag_name}");
    }

    fn notifications_allowed(&self) -> bool {
        true
    }

    fn request_notifications(&self) {}

    fn exact_alarms_allowed(&self) -> bool {
        true
    }

    fn request_exact_alarms(&self) {}

    fn share_text(&self, _title: &str, text: &str) -> Result<String, String> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let path = self.data_dir.join(format!("tempo-sessoes-{stamp}.csv"));
        std::fs::write(&path, text).map_err(|e| format!("Não foi possível exportar: {e}"))?;
        Ok(format!("Exportado para {}", path.display()))
    }
}
