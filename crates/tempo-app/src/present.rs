//! Conversão dos dados do serviço em estruturas da interface, com textos em pt-BR.

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate};
use slint::{Color, ModelRc, SharedString, VecModel};
use tempo_core::format::{day_title, hm, hms, short, short_date, tag_status};
use tempo_core::stats::TagStats;
use tempo_core::{Clock, Permille, Progress};
use tempo_store::{DaySummary, SessionView, StatsView, TagToday, TodayView};

use crate::{CalendarCell, DayInfo, HeroItem, HistoryDay, HistoryTag, ReservedItem, Segment, SessionItem, StatItem, TagItem, UsageTag};

pub fn color(c: u32) -> Color {
    Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn color_u32(c: Color) -> u32 {
    (u32::from(c.red()) << 16) | (u32::from(c.green()) << 8) | u32::from(c.blue())
}

pub fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

/// "HH:MM" local.
pub fn local_clock(clock: &dyn Clock, ts: i64) -> String {
    clock.utc_to_local(ts).format("%H:%M").to_string()
}

pub fn tag_item(t: &TagToday) -> TagItem {
    let p = t.progress();
    TagItem {
        id: t.tag.id as i32,
        name: t.tag.name.as_str().into(),
        color: color(t.tag.color),
        permille: i32::from(t.permille.value()),
        percent: t.permille.to_string().into(),
        allocated: short(t.allocated).into(),
        consumed: short(t.consumed).into(),
        status: tag_status(p).into(),
        progress: p.ratio(),
        running: t.running,
        finished: p.is_finished() && t.allocated > 0,
        excess: p.excess() > 0,
    }
}

/// Item de tag para a aba Planejar (porcentagem configurada da tag, não a de hoje).
pub fn plan_item(id: i64, name: &str, c: u32, p: Permille, available: i64) -> TagItem {
    TagItem {
        id: id as i32,
        name: name.into(),
        color: color(c),
        permille: i32::from(p.value()),
        percent: p.to_string().into(),
        allocated: hm(tempo_core::allocated_secs(available, p)).into(),
        ..Default::default()
    }
}

pub fn reserved_item(r: &tempo_store::Reserved) -> ReservedItem {
    ReservedItem {
        id: r.id as i32,
        name: r.name.as_str().into(),
        minutes: (r.duration / 60) as i32,
        duration: short(r.duration).into(),
    }
}

pub fn segments(v: &TodayView) -> Vec<Segment> {
    let mut offset = 0.0f32;
    v.tags
        .iter()
        .map(|t| {
            let fraction = t.permille.fraction();
            let s = Segment { color: color(t.tag.color), offset, fraction };
            offset += fraction;
            s
        })
        .collect()
}

pub fn day_info(v: &TodayView) -> DayInfo {
    DayInfo {
        title: day_title(v.date).into(),
        available: hm(v.available).into(),
        elapsed: short(v.day_elapsed).into(),
        day_left: short(v.day_remaining).into(),
        allocated_percent: v.allocated_permille.to_string().into(),
        unallocated_pct: v.unallocated_permille.to_string().into(),
        unallocated_time: short(v.unallocated_secs).into(),
        overcommitted: v.overcommitted,
        overcommit_text: format!(
            "Faltam {} nas tags e restam {} até o fim do dia.",
            short(v.total_remaining()),
            short(v.day_remaining)
        )
        .into(),
        clock_warning: v.clock_warning,
    }
}

/// Cartão principal: timer rodando ou tag pausada (foco).
pub fn hero(v: &TodayView, clock: &dyn Clock) -> HeroItem {
    let (tag, running) = match &v.active {
        Some(a) => (v.tags.iter().find(|t| t.tag.id == a.tag_id), true),
        None => (v.focus_tag.and_then(|id| v.tags.iter().find(|t| t.tag.id == id)), false),
    };
    let Some(t) = tag else { return HeroItem::default() };
    let p = t.progress();
    let (big, caption) = if p.excess() > 0 {
        (format!("+{}", hms(p.excess())), format!("Excedente · meta de {} cumprida", short(t.allocated)))
    } else if running {
        let ends = v.now + p.remaining();
        (hms(p.remaining()), format!("restantes de {} · termina às {}", short(t.allocated), local_clock(clock, ends)))
    } else {
        (hms(p.remaining()), format!("restantes de {}", short(t.allocated)))
    };
    HeroItem {
        visible: true,
        running,
        tag_id: t.tag.id as i32,
        name: t.tag.name.as_str().into(),
        color: color(t.tag.color),
        big_time: big.into(),
        caption: caption.into(),
        progress: p.ratio(),
        excess: p.excess() > 0,
    }
}

pub fn session_item(s: &SessionView, clock: &dyn Clock, now: i64) -> SessionItem {
    let end = s.end.unwrap_or(now);
    let range = match s.end {
        Some(e) => format!("{}–{}", local_clock(clock, s.start), local_clock(clock, e)),
        None => format!("{}–agora", local_clock(clock, s.start)),
    };
    SessionItem {
        id: s.id as i32,
        name: s.tag_name.as_str().into(),
        color: color(s.color),
        range: range.into(),
        duration: short((end - s.start).max(0)).into(),
        manual: s.origin == tempo_store::Origin::Manual,
        running: s.end.is_none(),
    }
}

pub fn history_day(d: &DaySummary) -> HistoryDay {
    let tags: Vec<HistoryTag> = d
        .tags
        .iter()
        .filter(|t| t.allocated > 0 || t.consumed > 0)
        .map(|t| {
            let p = Progress::new(t.allocated, t.consumed);
            HistoryTag {
                name: t.name.as_str().into(),
                color: color(t.color),
                text: format!("{} / {}", short(t.consumed), short(t.allocated)).into(),
                progress: p.ratio(),
                excess: p.excess() > 0,
            }
        })
        .collect();
    let subtitle = if d.no_record {
        "Sem registro".to_string()
    } else {
        format!("{} registrados de {} disponíveis", short(d.consumed_total()), short(d.available))
    };
    HistoryDay {
        title: short_date(d.date).into(),
        subtitle: subtitle.into(),
        today: !d.closed,
        no_record: d.no_record,
        tags: model(tags),
    }
}

pub fn stat_item(name: &str, c: u32, archived: bool, s: &TagStats) -> StatItem {
    let days = if s.days_planned == 1 { "1 dia".to_string() } else { format!("{} dias", s.days_planned) };
    StatItem {
        name: name.into(),
        color: color(c),
        compliance: format!("{}%", (s.compliance * 100.0).round() as i32).into(),
        progress: s.compliance,
        detail: format!(
            "Total {} · média {}/dia · planejada em {}",
            short(s.consumed_total),
            short(s.daily_average),
            days
        )
        .into(),
        archived,
    }
}

pub fn stats_caption(v: &StatsView) -> SharedString {
    match v.days {
        0 => "Ainda não há dias completos neste período. As estatísticas aparecem a partir de amanhã.".into(),
        1 => "Baseado em 1 dia completo.".into(),
        n => format!("Baseado em {n} dias completos.").into(),
    }
}

// -------- aba Período do histórico --------

/// Chave estável e ordenada de um dia, usada entre a interface e o Rust.
pub fn date_key(d: NaiveDate) -> i32 {
    d.year() * 10000 + d.month() as i32 * 100 + d.day() as i32
}

pub fn date_from_key(k: i32) -> Option<NaiveDate> {
    let y = k / 10000;
    let m = (k / 100 % 100) as u32;
    let d = (k % 100) as u32;
    NaiveDate::from_ymd_opt(y, m, d)
}

pub fn month_title(d: NaiveDate) -> String {
    const MESES: [&str; 12] = [
        "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro",
        "novembro", "dezembro",
    ];
    format!("{} de {}", MESES[(d.month() - 1) as usize], d.year())
}

pub fn shift_months(d: NaiveDate, delta: i32) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + delta;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}

pub fn month_bounds(month_start: NaiveDate) -> (NaiveDate, NaiveDate) {
    let next = shift_months(month_start, 1);
    (month_start, next - Duration::days(1))
}

/// 42 células (6 semanas × 7 dias), começando na segunda-feira da semana do primeiro dia do mês.
pub fn calendar_cells(month_start: NaiveDate, today: NaiveDate, selected: &BTreeSet<NaiveDate>) -> Vec<CalendarCell> {
    let first_dow = month_start.weekday().num_days_from_monday() as i64;
    let start = month_start - Duration::days(first_dow);
    (0..42)
        .map(|i| {
            let d = start + Duration::days(i);
            let outside = d.month() != month_start.month() || d.year() != month_start.year();
            CalendarCell {
                key: date_key(d),
                day: d.day() as i32,
                outside,
                future: d > today,
                today: d == today,
                selected: selected.contains(&d),
            }
        })
        .collect()
}

/// Soma do tempo consumido por tag nos dias selecionados do mês (reservado já foi removido).
pub struct PeriodUsage {
    pub available: i64,
    pub tags: Vec<(String, u32, i64)>, // nome, cor, consumido
    pub days: u32,
}

pub fn aggregate_period(summaries: &[DaySummary], selected: &BTreeSet<NaiveDate>) -> PeriodUsage {
    use std::collections::BTreeMap;
    let mut available = 0i64;
    let mut per_tag: BTreeMap<i64, (String, u32, i64)> = BTreeMap::new();
    let mut days = 0u32;
    for d in summaries {
        if !selected.contains(&d.date) {
            continue;
        }
        days += 1;
        available += d.available;
        for t in &d.tags {
            if t.consumed == 0 {
                continue;
            }
            let entry = per_tag.entry(t.tag_id).or_insert_with(|| (t.name.clone(), t.color, 0));
            entry.2 += t.consumed;
        }
    }
    let mut tags: Vec<(String, u32, i64)> = per_tag.into_values().collect();
    tags.sort_by(|a, b| b.2.cmp(&a.2));
    PeriodUsage { available, tags, days }
}

pub fn period_segments(usage: &PeriodUsage) -> Vec<Segment> {
    let mut offset = 0.0f32;
    let total = usage.available.max(1) as f32;
    usage
        .tags
        .iter()
        .map(|(_, c, consumed)| {
            let fraction = *consumed as f32 / total;
            let s = Segment { color: color(*c), offset, fraction };
            offset += fraction;
            s
        })
        .collect()
}

pub fn period_usage_tags(usage: &PeriodUsage) -> Vec<UsageTag> {
    let total = usage.available as f64;
    let mut used = 0i64;
    let mut rows: Vec<UsageTag> = usage
        .tags
        .iter()
        .map(|(name, c, consumed)| {
            used += *consumed;
            let pct = if total > 0.0 { *consumed as f64 / total * 100.0 } else { 0.0 };
            UsageTag {
                name: name.as_str().into(),
                color: color(*c),
                pct: format!("{}%", pct.round() as i32).into(),
                duration: short(*consumed).into(),
                muted: false,
            }
        })
        .collect();
    let unused = usage.available - used;
    if unused > 0 {
        let pct = unused as f64 / total * 100.0;
        rows.push(UsageTag {
            name: "Não usado".into(),
            color: Color::from_argb_u8(255, 160, 160, 170),
            pct: format!("{}%", pct.round() as i32).into(),
            duration: short(unused).into(),
            muted: true,
        });
    }
    rows
}

pub fn period_used_pct_text(usage: &PeriodUsage) -> String {
    if usage.available == 0 {
        return "—".into();
    }
    let used: i64 = usage.tags.iter().map(|(_, _, c)| *c).sum();
    let pct = used as f64 / usage.available as f64 * 100.0;
    format!("{}%", pct.round() as i32)
}

pub fn period_available_caption(usage: &PeriodUsage) -> String {
    format!(
        "{} tempo livre em {}",
        short(usage.available),
        if usage.days == 1 { "1 dia".to_string() } else { format!("{} dias", usage.days) }
    )
}

pub fn calendar_selection_caption(n: usize) -> String {
    match n {
        0 => "Nenhum dia selecionado. Toque nos dias pra escolher.".into(),
        1 => "1 dia selecionado.".into(),
        _ => format!("{n} dias selecionados."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cores_ida_e_volta() {
        assert_eq!(color_u32(color(0x3B82F6)), 0x3B82F6);
    }
}
