//! Estatísticas por período (RF31, RN29).

use std::collections::BTreeMap;

/// Uma tag em um dia do histórico.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayTagRecord {
    pub tag_id: i64,
    pub allocated: i64,
    pub consumed: i64,
}

/// Agregado de uma tag em um período.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TagStats {
    pub tag_id: i64,
    /// Dias do período em que a tag tinha alocação.
    pub days_planned: u32,
    pub allocated_total: i64,
    pub consumed_total: i64,
    /// Média do % de cumprimento diário (0 a 1), considerando só os dias com alocação.
    pub compliance: f32,
    /// Consumido médio por dia do período.
    pub daily_average: i64,
}

/// Cumprimento de um dia: `min(consumido ÷ alocado, 1)` (RN29).
pub fn day_compliance(allocated: i64, consumed: i64) -> f32 {
    if allocated <= 0 {
        return 0.0;
    }
    (consumed as f64 / allocated as f64).min(1.0) as f32
}

/// Agrega o histórico. `days` contém um item por dia do período (inclusive dias sem registro).
pub fn aggregate(days: &[Vec<DayTagRecord>]) -> Vec<TagStats> {
    let n_days = days.len().max(1) as i64;
    let mut acc: BTreeMap<i64, (u32, i64, i64, f64)> = BTreeMap::new();
    for day in days {
        for r in day {
            let e = acc.entry(r.tag_id).or_insert((0, 0, 0, 0.0));
            if r.allocated > 0 {
                e.0 += 1;
                e.3 += f64::from(day_compliance(r.allocated, r.consumed));
            }
            e.1 += r.allocated;
            e.2 += r.consumed;
        }
    }
    acc.into_iter()
        .map(|(tag_id, (planned, alloc, cons, comp_sum))| TagStats {
            tag_id,
            days_planned: planned,
            allocated_total: alloc,
            consumed_total: cons,
            compliance: if planned > 0 { (comp_sum / f64::from(planned)) as f32 } else { 0.0 },
            daily_average: cons / n_days,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(tag_id: i64, allocated: i64, consumed: i64) -> DayTagRecord {
        DayTagRecord { tag_id, allocated, consumed }
    }

    #[test]
    fn cumprimento_limitado_a_100() {
        assert_eq!(day_compliance(100, 150), 1.0);
        assert_eq!(day_compliance(100, 50), 0.5);
        assert_eq!(day_compliance(0, 50), 0.0);
    }

    #[test]
    fn agrega_periodo() {
        let days = vec![
            vec![r(1, 1000, 1000), r(2, 500, 0)],
            vec![r(1, 1000, 500), r(2, 500, 1000)],
            vec![], // dia sem registro
        ];
        let s = aggregate(&days);
        assert_eq!(s.len(), 2);
        let t1 = s[0];
        assert_eq!(t1.tag_id, 1);
        assert_eq!(t1.days_planned, 2);
        assert_eq!(t1.consumed_total, 1500);
        assert!((t1.compliance - 0.75).abs() < 1e-6);
        assert_eq!(t1.daily_average, 500);
        let t2 = s[1];
        assert!((t2.compliance - 0.5).abs() < 1e-6);
    }

    #[test]
    fn periodo_vazio() {
        assert!(aggregate(&[]).is_empty());
    }
}
