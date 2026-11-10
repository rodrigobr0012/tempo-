use crate::clock_time::ClockTime;
use crate::error::{CoreError, CoreResult};

pub const SECS_PER_DAY: i64 = 86_400;
const MIN_SLEEP_SECS: i64 = 3_600; // RN03: 1h
const MAX_SLEEP_SECS: i64 = 20 * 3_600; // RN03: 20h
const SHORT_SLEEP_SECS: i64 = 4 * 3_600;
const LONG_SLEEP_SECS: i64 = 12 * 3_600;

/// Aviso (não bloqueante) sobre a duração do sono — RN03.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepWarning {
    /// Menos de 4h de sono.
    Short,
    /// Mais de 12h de sono.
    Long,
}

impl SleepWarning {
    pub fn message(self) -> &'static str {
        match self {
            SleepWarning::Short => "Menos de 4h de sono é pouco. Tem certeza?",
            SleepWarning::Long => "Mais de 12h de sono reduz bastante o tempo disponível.",
        }
    }
}

/// Período de sono definido pelo usuário (RF01).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepSchedule {
    bedtime: ClockTime,
    wake: ClockTime,
}

impl SleepSchedule {
    /// Cria e valida o período de sono (RN02, RN03).
    pub fn new(bedtime: ClockTime, wake: ClockTime) -> CoreResult<Self> {
        if bedtime == wake {
            return Err(CoreError::SleepTimesEqual);
        }
        let s = Self { bedtime, wake };
        let secs = s.sleep_secs();
        if !(MIN_SLEEP_SECS..=MAX_SLEEP_SECS).contains(&secs) {
            return Err(CoreError::SleepOutOfRange);
        }
        Ok(s)
    }

    pub fn bedtime(&self) -> ClockTime {
        self.bedtime
    }

    pub fn wake(&self) -> ClockTime {
        self.wake
    }

    /// Duração do sono em segundos, considerando a virada da meia-noite (RN02).
    pub fn sleep_secs(&self) -> i64 {
        sleep_secs_between(self.bedtime, self.wake)
    }

    /// Tempo disponível do dia: 24h − sono (RN01).
    pub fn available_secs(&self) -> i64 {
        SECS_PER_DAY - self.sleep_secs()
    }

    pub fn warning(&self) -> Option<SleepWarning> {
        let s = self.sleep_secs();
        if s < SHORT_SLEEP_SECS {
            Some(SleepWarning::Short)
        } else if s > LONG_SLEEP_SECS {
            Some(SleepWarning::Long)
        } else {
            None
        }
    }
}

impl Default for SleepSchedule {
    /// 23:00 → 07:00 (8h de sono, 16h disponíveis).
    fn default() -> Self {
        Self { bedtime: ClockTime::new(23, 0).expect("válido"), wake: ClockTime::new(7, 0).expect("válido") }
    }
}

/// Duração entre dormir e acordar (RN02), sem validação. Útil para pré-visualizar na UI.
pub fn sleep_secs_between(bedtime: ClockTime, wake: ClockTime) -> i64 {
    let b = i64::from(bedtime.minutes());
    let w = i64::from(wake.minutes());
    let minutes = if w > b { w - b } else { 24 * 60 - b + w };
    minutes * 60
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u8, m: u8) -> ClockTime {
        ClockTime::new(h, m).unwrap()
    }

    #[test]
    fn exemplo_de_referencia_16h() {
        let s = SleepSchedule::new(t(23, 0), t(7, 0)).unwrap();
        assert_eq!(s.sleep_secs(), 8 * 3600);
        assert_eq!(s.available_secs(), 16 * 3600);
        assert_eq!(s.warning(), None);
    }

    #[test]
    fn sono_sem_cruzar_meia_noite() {
        let s = SleepSchedule::new(t(1, 0), t(9, 30)).unwrap();
        assert_eq!(s.sleep_secs(), 8 * 3600 + 1800);
    }

    #[test]
    fn horarios_iguais_sao_invalidos() {
        assert_eq!(SleepSchedule::new(t(7, 0), t(7, 0)), Err(CoreError::SleepTimesEqual));
    }

    #[test]
    fn limites_de_1h_a_20h() {
        assert!(SleepSchedule::new(t(23, 0), t(0, 0)).is_ok()); // 1h
        assert_eq!(SleepSchedule::new(t(23, 0), t(22, 59)), Err(CoreError::SleepOutOfRange)); // 23h59
        assert_eq!(SleepSchedule::new(t(23, 30), t(0, 0)), Err(CoreError::SleepOutOfRange)); // 30min
        assert!(SleepSchedule::new(t(0, 0), t(20, 0)).is_ok()); // 20h
        assert_eq!(SleepSchedule::new(t(0, 0), t(20, 1)), Err(CoreError::SleepOutOfRange));
    }

    #[test]
    fn avisos() {
        assert_eq!(SleepSchedule::new(t(3, 0), t(6, 0)).unwrap().warning(), Some(SleepWarning::Short));
        assert_eq!(SleepSchedule::new(t(20, 0), t(9, 0)).unwrap().warning(), Some(SleepWarning::Long));
        assert_eq!(SleepSchedule::new(t(0, 0), t(4, 0)).unwrap().warning(), None);
        assert_eq!(SleepSchedule::new(t(20, 0), t(8, 0)).unwrap().warning(), None);
    }
}
