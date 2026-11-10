//! Estado da versão web: o serviço `Tempo` (SQLite em memória) e os sinais da interface.
//!
//! O banco vive em memória no WebAssembly. Depois de cada alteração, uma cópia completa
//! (`Tempo::snapshot`, alguns KB) é gravada no IndexedDB. Ao abrir, a cópia é carregada de volta.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use leptos::task::spawn_local;
use tempo_core::SystemClock;
use tempo_store::{Error, Result, Tempo};

use crate::browser::{self, Permission};

thread_local! {
    static TEMPO: RefCell<Option<Tempo>> = const { RefCell::new(None) };
    /// Sessão cujo fim (meta cumprida) já foi avisado, para avisar uma vez só.
    static WARNED_SESSION: Cell<i64> = const { Cell::new(-1) };
    static TOAST_SEQ: Cell<u64> = const { Cell::new(0) };
    /// Último timer enviado ao programa para Windows (sessão, prazo), para não repetir.
    static SYNCED: Cell<Option<(i64, i64)>> = const { Cell::new(None) };
}

/// Abre o serviço a partir dos bytes salvos (vazio = banco novo).
pub fn load(bytes: &[u8]) -> Result<()> {
    let t = Tempo::from_snapshot(bytes, Arc::new(SystemClock))?;
    TEMPO.with(|c| *c.borrow_mut() = Some(t));
    Ok(())
}

/// Executa `f` com acesso exclusivo ao serviço. Não altere sinais dentro de `f`.
pub fn with<R>(f: impl FnOnce(&mut Tempo) -> Result<R>) -> Result<R> {
    TEMPO.with(|c| match c.borrow_mut().as_mut() {
        Some(t) => f(t),
        None => Err(Error::NotOnboarded),
    })
}

/// Programa para Windows: envia o timer atual ao processo nativo (bandeja + aviso de fim),
/// só quando ele muda.
pub fn sync_desktop() {
    if !browser::is_desktop() {
        return;
    }
    let active = with(|t| t.active_timer()).ok().flatten();
    let key = active.as_ref().map(|a| (a.session_id, a.deadline));
    if SYNCED.with(|s| s.replace(key)) == key {
        return;
    }
    match active {
        Some(a) => {
            let tooltip = if a.start < a.deadline {
                format!("{} · termina às {}", a.tag_name, crate::ui::local_clock(a.deadline))
            } else {
                format!("{} · excedente", a.tag_name)
            };
            browser::sync_timer(Some((&a.tag_name, a.deadline * 1000, &tooltip)));
        }
        None => browser::sync_timer(None),
    }
}

/// Grava a cópia do banco no IndexedDB (em segundo plano).
pub fn persist() {
    let Ok(bytes) = with(|t| t.snapshot()) else { return };
    spawn_local(async move {
        if let Err(e) = browser::save_db(&bytes).await {
            leptos::logging::error!("não foi possível salvar: {e}");
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Today,
    Plan,
    History,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryTab {
    Days,
    Week,
    Month,
    Period,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    Erase,
    DeleteTag(i64),
    DeleteReserved(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// `id = None` cria uma tag nova.
    TagEditor {
        id: Option<i64>,
    },
    Manual,
    Sessions,
    Reserved {
        id: Option<i64>,
    },
    Confirm {
        title: String,
        text: String,
        button: String,
        action: Confirm,
    },
}

/// Sinais compartilhados pela interface (copiável, fornecido via contexto).
#[derive(Clone, Copy)]
pub struct Ctx {
    /// Muda a cada alteração no banco: as consultas dependem dele.
    pub rev: RwSignal<u64>,
    /// "Agora" em segundos Unix, atualizado a cada segundo.
    pub now: RwSignal<i64>,
    pub page: RwSignal<Page>,
    pub history_tab: RwSignal<HistoryTab>,
    /// Primeiro dia do mês mostrado na grade do calendário (aba "Período").
    pub calendar_month: RwSignal<NaiveDate>,
    /// Dias que o usuário pintou no calendário para o cálculo do período.
    pub calendar_selected: RwSignal<BTreeSet<NaiveDate>>,
    pub dialog: RwSignal<Option<Dialog>>,
    pub toast: RwSignal<Option<(u64, String)>>,
    pub notif: RwSignal<Permission>,
}

pub fn ctx() -> Ctx {
    expect_context::<Ctx>()
}

impl Ctx {
    pub fn new() -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            rev: RwSignal::new(0),
            now: RwSignal::new(chrono::Utc::now().timestamp()),
            page: RwSignal::new(Page::Today),
            history_tab: RwSignal::new(HistoryTab::Days),
            calendar_month: RwSignal::new(NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today)),
            calendar_selected: RwSignal::new(BTreeSet::new()),
            dialog: RwSignal::new(None),
            toast: RwSignal::new(None),
            notif: RwSignal::new(browser::notification_permission()),
        }
    }

    /// Consulta que se refaz quando o banco muda.
    pub fn read<R>(&self, f: impl FnOnce(&mut Tempo) -> Result<R>) -> Option<R> {
        self.rev.track();
        match with(f) {
            Ok(r) => Some(r),
            Err(Error::NotOnboarded) => None,
            Err(e) => {
                leptos::logging::warn!("{e}");
                None
            }
        }
    }

    /// Executa uma alteração. Sucesso: atualiza a tela e salva. Erro: vira aviso na tela.
    pub fn act<R>(&self, f: impl FnOnce(&mut Tempo) -> Result<R>) -> Option<R> {
        match with(f) {
            Ok(r) => {
                self.changed();
                Some(r)
            }
            Err(e) => {
                self.say(e.to_string());
                None
            }
        }
    }

    /// O banco mudou: refaz as consultas e grava a cópia.
    pub fn changed(&self) {
        self.rev.update(|r| *r += 1);
        persist();
        sync_desktop();
    }

    /// Outra aba salvou: recarrega a cópia dela.
    pub fn reload_from(&self, bytes: &[u8]) {
        if load(bytes).is_ok() {
            self.rev.update(|r| *r += 1);
            sync_desktop();
        }
    }

    /// Aviso temporário na parte de baixo da tela.
    pub fn say(&self, msg: impl Into<String>) {
        let id = TOAST_SEQ.with(|s| {
            s.set(s.get() + 1);
            s.get()
        });
        self.toast.set(Some((id, msg.into())));
        let toast = self.toast;
        set_timeout(
            move || {
                if toast.get_untracked().is_some_and(|(i, _)| i == id) {
                    toast.set(None);
                }
            },
            Duration::from_secs(4),
        );
    }

    pub fn confirm(&self, title: impl Into<String>, text: impl Into<String>, button: &str, action: Confirm) {
        self.dialog.set(Some(Dialog::Confirm {
            title: title.into(),
            text: text.into(),
            button: button.into(),
            action,
        }));
    }

    /// Executado a cada segundo: vira o dia, encerra timers esgotados e avisa o fim da meta.
    pub fn tick(&self) {
        match with(|t| t.tick()) {
            Ok(report) => {
                if report.changed() {
                    self.changed();
                }
                if let Some(tag_id) = report.auto_stopped {
                    if let Ok(name) = with(|t| Ok(t.tag(tag_id)?.name)) {
                        self.finished(&name);
                    }
                }
            }
            Err(Error::NotOnboarded) => {}
            Err(e) => leptos::logging::warn!("tick: {e}"),
        }
        // Com o excedente ligado o timer não para: avisa uma vez quando a meta é cumprida (RF20).
        let now = chrono::Utc::now().timestamp();
        if let Ok(Some(a)) = with(|t| t.active_timer()) {
            let crossed = a.start < a.deadline && now >= a.deadline;
            if crossed && WARNED_SESSION.with(Cell::get) != a.session_id {
                WARNED_SESSION.with(|w| w.set(a.session_id));
                self.finished(&a.tag_name);
            }
        }
        self.now.set(now);
    }

    fn finished(&self, tag_name: &str) {
        self.say(format!("Tempo de \"{tag_name}\" esgotado."));
        if browser::is_desktop() {
            // No programa para Windows o aviso (com som) vem do processo nativo, no horário exato.
            return;
        }
        let (sound, vibration) = with(|t| t.settings()).map(|s| (s.sound, s.vibration)).unwrap_or((true, true));
        browser::notify("Tempo esgotado", &format!("O tempo de \"{tag_name}\" acabou."));
        if sound {
            browser::beep();
        }
        if vibration {
            browser::vibrate();
        }
    }

    pub fn request_notifications(&self) {
        if browser::is_desktop() {
            return;
        }
        let notif = self.notif;
        spawn_local(async move {
            notif.set(browser::request_notifications().await);
        });
    }
}
