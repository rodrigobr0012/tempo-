//! Regras de negócio do app de gestão de tempo por disponibilidade.
//!
//! Este crate é Rust puro, sem dependência de Android, e roda os testes no desktop (RNF05).
//! Referências `RNxx`/`RFxx` apontam para `documentação.md`.

pub mod clock_time;
pub mod day;
pub mod error;
pub mod format;
pub mod percent;
pub mod reserved;
pub mod stats;
pub mod tags;
pub mod timer;

pub use clock_time::ClockTime;
pub use day::{Clock, DayBounds, ManualClock, SystemClock};
pub use error::{CoreError, CoreResult};
pub use percent::{allocated_secs, Permille};
pub use reserved::{available_secs, SECS_PER_DAY};
pub use timer::{Progress, Span};
