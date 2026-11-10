//! Tempo reservado (RF01–RF03, RN01–RN03).
//!
//! O usuário cria blocos com nome e duração (ex.: "Sono 8h", "Faculdade 4h").
//! O tempo disponível do dia é `24h − soma dos blocos`, e é ele que as tags dividem em %.

use crate::error::{CoreError, CoreResult};
use crate::format;

pub const SECS_PER_DAY: i64 = 86_400;
/// RN02: os blocos podem somar no máximo 23h, para sobrar ao menos 1h disponível.
pub const MAX_RESERVED_SECS: i64 = 23 * 3_600;
/// RN03: limite de blocos.
pub const MAX_RESERVED_BLOCKS: usize = 20;
/// Bloco sugerido na primeira execução (RF04).
pub const DEFAULT_RESERVED: [(&str, i64); 1] = [("Sono", 8 * 3_600)];
/// RN25: horário padrão de início do dia lógico.
pub const DEFAULT_DAY_START: (u8, u8) = (4, 0);

/// Tempo disponível: `24h − reservado` (RN01).
pub fn available_secs(reserved_total: i64) -> i64 {
    (SECS_PER_DAY - reserved_total.max(0)).max(0)
}

/// Valida a duração de um bloco dado o total dos outros blocos (RN02).
pub fn check_reserved(others_total: i64, duration_secs: i64) -> CoreResult<()> {
    if duration_secs <= 0 {
        return Err(CoreError::DurationZero);
    }
    if others_total + duration_secs > MAX_RESERVED_SECS {
        let left = (MAX_RESERVED_SECS - others_total).max(0);
        return Err(CoreError::ReservedExceeded { available: format::short(left) });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600;

    #[test]
    fn exemplo_de_referencia() {
        assert_eq!(available_secs(8 * H), 16 * H);
        assert_eq!(available_secs(8 * H + 4 * H), 12 * H);
        assert_eq!(available_secs(0), 24 * H);
    }

    #[test]
    fn limite_de_23h() {
        assert!(check_reserved(20 * H, 3 * H).is_ok());
        let err = check_reserved(20 * H, 3 * H + 60).unwrap_err();
        assert_eq!(err, CoreError::ReservedExceeded { available: "3h".into() });
        assert_eq!(check_reserved(0, 0), Err(CoreError::DurationZero));
    }
}
