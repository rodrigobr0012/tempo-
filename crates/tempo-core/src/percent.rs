use std::fmt;
use std::str::FromStr;

use crate::error::{CoreError, CoreResult};

/// Porcentagem com precisão de 0,1%, guardada como inteiro em milésimos (RN05).
///
/// `Permille(100)` = 10%, `Permille(1000)` = 100%.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Permille(u16);

impl Permille {
    pub const ZERO: Permille = Permille(0);
    pub const MIN: Permille = Permille(1);
    pub const FULL: Permille = Permille(1000);

    /// Valor de alocação válido para uma tag ativa: 0,1% a 100% (RN10).
    pub fn new(value: u16) -> CoreResult<Self> {
        if !(1..=1000).contains(&value) {
            return Err(CoreError::InvalidPercent);
        }
        Ok(Self(value))
    }

    /// Valor sem validação de mínimo (0 a 1000). Usado para somas e "Não alocado".
    pub fn saturating(value: i64) -> Self {
        Self(value.clamp(0, 1000) as u16)
    }

    pub fn value(self) -> u16 {
        self.0
    }

    /// Fração entre 0 e 1.
    pub fn fraction(self) -> f32 {
        f32::from(self.0) / 1000.0
    }

    /// Converte uma duração em porcentagem do tempo disponível, arredondando a 0,1% (RN07).
    pub fn from_duration(duration_secs: i64, available_secs: i64) -> CoreResult<Self> {
        if duration_secs <= 0 {
            return Err(CoreError::DurationZero);
        }
        if available_secs <= 0 {
            return Err(CoreError::InvalidPercent);
        }
        // arredondamento "meio para cima" em inteiros
        let v = (duration_secs * 1000 + available_secs / 2) / available_secs;
        if v > 1000 {
            return Err(CoreError::InvalidPercent);
        }
        Self::new(v.max(1) as u16)
    }
}

/// Tempo alocado em segundos: `floor(TD × porcentagem)` (RN04).
pub fn allocated_secs(available_secs: i64, p: Permille) -> i64 {
    available_secs.max(0) * i64::from(p.0) / 1000
}

impl fmt::Display for Permille {
    /// Formato pt-BR: "10%", "12,3%".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int = self.0 / 10;
        let dec = self.0 % 10;
        if dec == 0 {
            write!(f, "{int}%")
        } else {
            write!(f, "{int},{dec}%")
        }
    }
}

impl FromStr for Permille {
    type Err = CoreError;

    /// Aceita "10", "10%", "12,3", "12.3".
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().trim_end_matches('%').trim().replace(',', ".");
        let (int, dec) = match s.split_once('.') {
            Some((i, d)) => (i, d),
            None => (s.as_str(), ""),
        };
        if int.is_empty() && dec.is_empty() {
            return Err(CoreError::InvalidPercent);
        }
        let int: u16 = if int.is_empty() { 0 } else { int.parse().map_err(|_| CoreError::InvalidPercent)? };
        if dec.len() > 1 || (!dec.is_empty() && !dec.bytes().all(|b| b.is_ascii_digit())) {
            return Err(CoreError::InvalidPercent);
        }
        let dec: u16 = if dec.is_empty() { 0 } else { dec.parse().map_err(|_| CoreError::InvalidPercent)? };
        let value = int.checked_mul(10).and_then(|v| v.checked_add(dec)).ok_or(CoreError::InvalidPercent)?;
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TD_16H: i64 = 16 * 3600;

    #[test]
    fn exemplos_da_documentacao() {
        assert_eq!(allocated_secs(TD_16H, Permille::new(100).unwrap()), 5_760); // 10% = 1h36
        assert_eq!(allocated_secs(TD_16H, Permille::new(500).unwrap()), 28_800); // 50% = 8h
        assert_eq!(allocated_secs(TD_16H, Permille::new(50).unwrap()), 2_880); // 5% = 48min
        assert_eq!(allocated_secs(TD_16H, Permille::new(123).unwrap()), 7_084); // 12,3% → floor
    }

    #[test]
    fn soma_dos_alocados_nunca_passa_do_td() {
        // RN06: 3 × 33,3% + 0,1% = 100%
        let parts = [333u16, 333, 333, 1];
        let td = 57_601;
        let total: i64 = parts.iter().map(|&p| allocated_secs(td, Permille::new(p).unwrap())).sum();
        assert!(total <= td);
    }

    #[test]
    fn duracao_para_porcentagem() {
        assert_eq!(Permille::from_duration(5_760, TD_16H).unwrap().value(), 100);
        assert_eq!(Permille::from_duration(2 * 3600, TD_16H).unwrap().value(), 125); // 12,5%
        assert_eq!(Permille::from_duration(1, TD_16H).unwrap().value(), 1); // mínimo 0,1%
        assert_eq!(Permille::from_duration(0, TD_16H), Err(CoreError::DurationZero));
        assert_eq!(Permille::from_duration(TD_16H + 3600, TD_16H), Err(CoreError::InvalidPercent));
    }

    #[test]
    fn parse_e_display() {
        assert_eq!("10".parse::<Permille>().unwrap().value(), 100);
        assert_eq!("12,3%".parse::<Permille>().unwrap().value(), 123);
        assert_eq!(" 7.5 ".parse::<Permille>().unwrap().value(), 75);
        assert_eq!(",5".parse::<Permille>().unwrap().value(), 5);
        assert!("12,34".parse::<Permille>().is_err());
        assert!("0".parse::<Permille>().is_err());
        assert!("100,1".parse::<Permille>().is_err());
        assert!("abc".parse::<Permille>().is_err());
        assert!("".parse::<Permille>().is_err());
        assert_eq!(Permille::new(100).unwrap().to_string(), "10%");
        assert_eq!(Permille::new(123).unwrap().to_string(), "12,3%");
        assert_eq!(Permille::new(5).unwrap().to_string(), "0,5%");
    }
}
