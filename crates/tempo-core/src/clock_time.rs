use std::fmt;
use std::str::FromStr;

use chrono::NaiveTime;

use crate::error::{CoreError, CoreResult};

/// Minutos em um dia.
pub const MINUTES_PER_DAY: u16 = 24 * 60;

/// Horário do relógio com precisão de minutos (00:00 a 23:59).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockTime(u16);

impl ClockTime {
    pub fn new(hour: u8, minute: u8) -> CoreResult<Self> {
        if hour > 23 || minute > 59 {
            return Err(CoreError::InvalidClockTime);
        }
        Ok(Self(u16::from(hour) * 60 + u16::from(minute)))
    }

    pub fn from_minutes(minutes: u16) -> CoreResult<Self> {
        if minutes >= MINUTES_PER_DAY {
            return Err(CoreError::InvalidClockTime);
        }
        Ok(Self(minutes))
    }

    /// Minutos desde a meia-noite.
    pub fn minutes(self) -> u16 {
        self.0
    }

    pub fn hour(self) -> u8 {
        (self.0 / 60) as u8
    }

    pub fn minute(self) -> u8 {
        (self.0 % 60) as u8
    }

    /// Soma (ou subtrai) minutos dando a volta no relógio.
    pub fn add_minutes(self, delta: i32) -> Self {
        let total = i32::from(MINUTES_PER_DAY);
        let m = (i32::from(self.0) + delta).rem_euclid(total);
        Self(m as u16)
    }

    pub fn to_naive_time(self) -> NaiveTime {
        NaiveTime::from_hms_opt(u32::from(self.hour()), u32::from(self.minute()), 0).expect("ClockTime sempre é válido")
    }
}

impl fmt::Display for ClockTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour(), self.minute())
    }
}

impl FromStr for ClockTime {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (h, m) = s.trim().split_once(':').ok_or(CoreError::InvalidClockTime)?;
        let h: u8 = h.parse().map_err(|_| CoreError::InvalidClockTime)?;
        let m: u8 = m.parse().map_err(|_| CoreError::InvalidClockTime)?;
        Self::new(h, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_limites() {
        assert!(ClockTime::new(23, 59).is_ok());
        assert_eq!(ClockTime::new(24, 0), Err(CoreError::InvalidClockTime));
        assert_eq!(ClockTime::new(10, 60), Err(CoreError::InvalidClockTime));
        assert_eq!(ClockTime::from_minutes(1440), Err(CoreError::InvalidClockTime));
    }

    #[test]
    fn parse_e_display() {
        let t: ClockTime = "07:05".parse().unwrap();
        assert_eq!(t.hour(), 7);
        assert_eq!(t.minute(), 5);
        assert_eq!(t.to_string(), "07:05");
        assert!("7h".parse::<ClockTime>().is_err());
        assert!("aa:00".parse::<ClockTime>().is_err());
    }

    #[test]
    fn soma_da_volta_no_relogio() {
        let t = ClockTime::new(23, 30).unwrap();
        assert_eq!(t.add_minutes(45).to_string(), "00:15");
        assert_eq!(t.add_minutes(-24 * 60).to_string(), "23:30");
        assert_eq!(ClockTime::new(0, 10).unwrap().add_minutes(-20).to_string(), "23:50");
    }
}
