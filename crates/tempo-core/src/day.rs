//! Relógio e dia lógico (RN25).
//!
//! O dia lógico começa no horário de acordar e termina no horário de acordar seguinte.
//! Instantes são guardados como segundos Unix (UTC); a conversão para o horário local
//! passa pelo trait [`Clock`], o que permite testar com relógio manual.

use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::clock_time::ClockTime;

/// Fonte de tempo e conversão local ↔ UTC.
pub trait Clock: Send + Sync {
    /// Agora, em segundos Unix (UTC).
    fn now(&self) -> i64;
    /// Converte um instante para data/hora local.
    fn utc_to_local(&self, ts: i64) -> NaiveDateTime;
    /// Converte uma data/hora local para instante. Em horários ambíguos (fim do horário
    /// de verão) usa o mais cedo; em horários inexistentes, avança até existir.
    fn local_to_utc(&self, dt: NaiveDateTime) -> i64;
}

/// Relógio do sistema, no fuso local do aparelho.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }

    fn utc_to_local(&self, ts: i64) -> NaiveDateTime {
        match Local.timestamp_opt(ts, 0).earliest() {
            Some(dt) => dt.naive_local(),
            None => utc_naive(ts),
        }
    }

    fn local_to_utc(&self, dt: NaiveDateTime) -> i64 {
        // Lacuna de horário de verão: tenta até 3h à frente.
        for shift in 0..=3 {
            let probe = dt + chrono::Duration::hours(shift);
            if let Some(t) = Local.from_local_datetime(&probe).earliest() {
                return t.timestamp() - shift * 3600;
            }
        }
        dt.and_utc().timestamp()
    }
}

/// Relógio controlado manualmente, com fuso fixo. Para testes e simulações.
#[derive(Debug)]
pub struct ManualClock {
    now: AtomicI64,
    offset_secs: i64,
}

impl ManualClock {
    pub fn new(now: i64, offset_secs: i64) -> Self {
        Self { now: AtomicI64::new(now), offset_secs }
    }

    /// Cria o relógio a partir de uma data/hora local (UTC−3 por padrão nos testes).
    pub fn at_local(dt: NaiveDateTime, offset_secs: i64) -> Self {
        Self::new(dt.and_utc().timestamp() - offset_secs, offset_secs)
    }

    pub fn set(&self, now: i64) {
        self.now.store(now, Ordering::SeqCst);
    }

    pub fn advance(&self, secs: i64) {
        self.now.fetch_add(secs, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }

    fn utc_to_local(&self, ts: i64) -> NaiveDateTime {
        utc_naive(ts + self.offset_secs)
    }

    fn local_to_utc(&self, dt: NaiveDateTime) -> i64 {
        dt.and_utc().timestamp() - self.offset_secs
    }
}

impl<C: Clock + ?Sized> Clock for std::sync::Arc<C> {
    fn now(&self) -> i64 {
        (**self).now()
    }
    fn utc_to_local(&self, ts: i64) -> NaiveDateTime {
        (**self).utc_to_local(ts)
    }
    fn local_to_utc(&self, dt: NaiveDateTime) -> i64 {
        (**self).local_to_utc(dt)
    }
}

fn utc_naive(ts: i64) -> NaiveDateTime {
    DateTime::from_timestamp(ts, 0).unwrap_or_default().naive_utc()
}

/// Limites de um dia lógico.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayBounds {
    /// Data (local) em que o dia lógico começou — identifica o dia.
    pub date: NaiveDate,
    /// Início (horário de acordar), em segundos Unix.
    pub start: i64,
    /// Fim (próximo horário de acordar), em segundos Unix.
    pub end: i64,
}

/// Limites do dia lógico que começa na data `date`, com o horário de acordar `wake`.
pub fn day_bounds_for_date(clock: &dyn Clock, date: NaiveDate, wake: ClockTime) -> DayBounds {
    let wake_t = wake.to_naive_time();
    let start = clock.local_to_utc(date.and_time(wake_t));
    let next = date.succ_opt().unwrap_or(date);
    let end = clock.local_to_utc(next.and_time(wake_t)).max(start + 1);
    DayBounds { date, start, end }
}

/// Dia lógico que contém o instante `ts` (RN25).
pub fn day_bounds_containing(clock: &dyn Clock, ts: i64, wake: ClockTime) -> DayBounds {
    let local = clock.utc_to_local(ts);
    let mut date = local.date();
    if local.time() < wake.to_naive_time() {
        date = date.pred_opt().unwrap_or(date);
    }
    let b = day_bounds_for_date(clock, date, wake);
    // Proteção contra bordas de horário de verão: garante start <= ts < end.
    DayBounds { date: b.date, start: b.start.min(ts), end: b.end.max(ts + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const BRT: i64 = -3 * 3600;

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    #[test]
    fn dia_comeca_no_horario_de_acordar() {
        let clock = ManualClock::at_local(local(2026, 11, 9, 10, 0), BRT);
        let wake = ClockTime::new(7, 0).unwrap();
        let b = day_bounds_containing(&clock, clock.now(), wake);
        assert_eq!(b.date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        assert_eq!(clock.utc_to_local(b.start), local(2026, 11, 9, 7, 0));
        assert_eq!(clock.utc_to_local(b.end), local(2026, 11, 10, 7, 0));
        assert_eq!(b.end - b.start, 86_400);
    }

    #[test]
    fn madrugada_pertence_ao_dia_anterior() {
        let clock = ManualClock::at_local(local(2026, 11, 10, 2, 30), BRT);
        let wake = ClockTime::new(7, 0).unwrap();
        let b = day_bounds_containing(&clock, clock.now(), wake);
        assert_eq!(b.date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
    }

    #[test]
    fn exatamente_no_horario_de_acordar_comeca_novo_dia() {
        let clock = ManualClock::at_local(local(2026, 11, 10, 7, 0), BRT);
        let wake = ClockTime::new(7, 0).unwrap();
        let b = day_bounds_containing(&clock, clock.now(), wake);
        assert_eq!(b.date, NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
        assert_eq!(b.start, clock.now());
    }

    #[test]
    fn relogio_manual_avanca() {
        let clock = ManualClock::new(1_000, 0);
        clock.advance(60);
        assert_eq!(clock.now(), 1_060);
        clock.set(5);
        assert_eq!(clock.now(), 5);
        assert_eq!(clock.local_to_utc(clock.utc_to_local(12_345)), 12_345);
    }

    #[test]
    fn relogio_do_sistema_ida_e_volta() {
        let c = SystemClock;
        let now = c.now();
        assert!(now > 1_700_000_000);
        assert_eq!(c.local_to_utc(c.utc_to_local(now)), now);
    }
}
