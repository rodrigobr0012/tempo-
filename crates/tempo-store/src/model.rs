//! Tipos expostos pelo serviço para a interface.

use chrono::NaiveDate;
use tempo_core::{Permille, Progress, SleepSchedule};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub schedule: SleepSchedule,
    /// RF21: depois de zerar, o timer continua contando como excedente.
    pub overtime: bool,
    pub sound: bool,
    pub vibration: bool,
    /// RN30: `None` = manter para sempre.
    pub retention_days: Option<u32>,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schedule: SleepSchedule::default(),
            overtime: true,
            sound: true,
            vibration: true,
            retention_days: None,
            onboarded: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    /// 0xRRGGBB
    pub color: u32,
    pub permille: Permille,
    pub position: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTag {
    pub name: String,
    pub color: u32,
    pub permille: Permille,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Timer,
    Manual,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Timer => "timer",
            Origin::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "manual" {
            Origin::Manual
        } else {
            Origin::Timer
        }
    }
}

/// Uma tag no painel do dia.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagToday {
    pub tag: Tag,
    /// Porcentagem vigente hoje.
    pub permille: Permille,
    pub allocated: i64,
    pub consumed: i64,
    pub running: bool,
}

impl TagToday {
    pub fn progress(&self) -> Progress {
        Progress::new(self.allocated, self.consumed)
    }
}

/// Timer em andamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveTimer {
    pub session_id: i64,
    pub tag_id: i64,
    pub tag_name: String,
    pub color: u32,
    /// Início da sessão atual.
    pub start: i64,
    /// Consumido pela tag hoje antes desta sessão.
    pub consumed_before: i64,
    pub allocated: i64,
    /// Instante em que o consumido atinge o alocado.
    pub deadline: i64,
}

impl ActiveTimer {
    pub fn consumed(&self, now: i64) -> i64 {
        self.consumed_before + (now - self.start).max(0)
    }

    pub fn progress(&self, now: i64) -> Progress {
        Progress::new(self.allocated, self.consumed(now))
    }
}

/// Painel do dia (RF25–RF27).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodayView {
    pub date: NaiveDate,
    pub now: i64,
    pub day_start: i64,
    pub day_end: i64,
    /// TD do dia (snapshot).
    pub available: i64,
    /// Tempo desde o horário de acordar.
    pub awake_elapsed: i64,
    /// Tempo até o horário de dormir (início + TD), mínimo 0.
    pub awake_remaining: i64,
    pub tags: Vec<TagToday>,
    pub allocated_permille: Permille,
    pub unallocated_permille: Permille,
    pub unallocated_secs: i64,
    pub active: Option<ActiveTimer>,
    /// Tag pausada/selecionada no cartão principal.
    pub focus_tag: Option<i64>,
    /// RF27: a soma dos restantes não cabe mais no tempo acordado que sobra.
    pub overcommitted: bool,
    /// RN24: o relógio do aparelho voltou e a sessão foi encerrada.
    pub clock_warning: bool,
}

impl TodayView {
    pub fn total_remaining(&self) -> i64 {
        self.tags.iter().map(|t| t.progress().remaining()).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionView {
    pub id: i64,
    pub tag_id: i64,
    pub tag_name: String,
    pub color: u32,
    pub start: i64,
    pub end: Option<i64>,
    pub origin: Origin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayTagSummary {
    pub tag_id: i64,
    pub name: String,
    pub color: u32,
    pub permille: Permille,
    pub allocated: i64,
    pub consumed: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaySummary {
    pub date: NaiveDate,
    pub available: i64,
    pub closed: bool,
    pub no_record: bool,
    pub tags: Vec<DayTagSummary>,
}

impl DaySummary {
    pub fn consumed_total(&self) -> i64 {
        self.tags.iter().map(|t| t.consumed).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagStatsView {
    pub tag_id: i64,
    pub name: String,
    pub color: u32,
    pub archived: bool,
    pub stats: tempo_core::stats::TagStats,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatsView {
    /// Quantos dias fechados entraram no cálculo.
    pub days: u32,
    pub tags: Vec<TagStatsView>,
}

/// Resultado de [`crate::Tempo::tick`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub day_rolled: bool,
    /// Tag cujo timer foi encerrado automaticamente ao zerar (RN20).
    pub auto_stopped: Option<i64>,
    pub clock_went_back: bool,
}

impl TickReport {
    pub fn changed(&self) -> bool {
        self.day_rolled || self.auto_stopped.is_some() || self.clock_went_back
    }
}
