//! Conversão dos dados do serviço em estruturas da interface, com textos em pt-BR.

use chrono::{Datelike, NaiveDate};
use slint::{Color, ModelRc, SharedString, VecModel};
use tempo_core::format::{hm, hms, short};
use tempo_core::stats::TagStats;
use tempo_core::{Clock, Permille, Progress};
use tempo_store::{DaySummary, SessionView, StatsView, TagToday, TodayView};

use crate::{DayInfo, HeroItem, HistoryDay, HistoryTag, Segment, SessionItem, StatItem, TagItem};

const WEEKDAYS: [&str; 7] =
    ["segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado", "domingo"];
const WEEKDAYS_SHORT: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
const MONTHS: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

pub fn color(c: u32) -> Color {
    Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn color_u32(c: Color) -> u32 {
    (u32::from(c.red()) << 16) | (u32::from(c.green()) << 8) | u32::from(c.blue())
}

pub fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// "Segunda-feira, 9 de novembro"
pub fn day_title(d: NaiveDate) -> String {
    let wd = WEEKDAYS[d.weekday().num_days_from_monday() as usize];
    format!("{}, {} de {}", capitalize(wd), d.day(), MONTHS[d.month0() as usize])
}

/// "Seg, 09/11"
pub fn short_date(d: NaiveDate) -> String {
    format!("{}, {:02}/{:02}", WEEKDAYS_SHORT[d.weekday().num_days_from_monday() as usize], d.day(), d.month())
}

/// "HH:MM" local.
pub fn local_clock(clock: &dyn Clock, ts: i64) -> String {
    clock.utc_to_local(ts).format("%H:%M").to_string()
}

pub fn tag_status(p: Progress) -> String {
    if p.excess() > 0 {
        format!("+{} de excedente", short(p.excess()))
    } else if p.is_finished() {
        "Concluído".into()
    } else {
        format!("Restam {}", short(p.remaining()))
    }
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
        elapsed: short(v.awake_elapsed).into(),
        until_sleep: short(v.awake_remaining).into(),
        allocated_percent: v.allocated_permille.to_string().into(),
        unallocated: format!("{} · {}", v.unallocated_permille, short(v.unallocated_secs)).into(),
        overcommitted: v.overcommitted,
        overcommit_text: format!(
            "Faltam {} nas tags e restam {} até a hora de dormir.",
            short(v.total_remaining()),
            short(v.awake_remaining)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas_em_portugues() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(day_title(d), "Segunda-feira, 9 de novembro");
        assert_eq!(short_date(d), "Seg, 09/11");
        assert_eq!(short_date(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()), "Sáb, 14/11");
    }

    #[test]
    fn cores_ida_e_volta() {
        assert_eq!(color_u32(color(0x3B82F6)), 0x3B82F6);
    }

    #[test]
    fn status_da_tag() {
        assert_eq!(tag_status(Progress::new(5760, 600)), "Restam 1h26min");
        assert_eq!(tag_status(Progress::new(600, 600)), "Concluído");
        assert_eq!(tag_status(Progress::new(600, 1320)), "+12min de excedente");
    }
}
