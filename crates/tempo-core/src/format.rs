//! Formatação de durações (RN08) e textos de data/status em pt-BR, compartilhados pelo app
//! (Slint) e pela versão web.

use chrono::{Datelike, NaiveDate};

use crate::timer::Progress;

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

/// `HH:MM:SS` — usado no timer. Valores negativos viram zero.
pub fn hms(secs: i64) -> String {
    let s = secs.max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// `XhYYmin` com minutos truncados — usado em resumos e listas.
pub fn hm(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}h{:02}min", s / 3600, (s % 3600) / 60)
}

/// Forma curta para textos corridos: "1h36min", "48min", "2h".
pub fn short(secs: i64) -> String {
    let s = secs.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    match (h, m) {
        (0, m) => format!("{m}min"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m:02}min"),
    }
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

/// "Restam 1h26min", "Concluído" ou "+12min de excedente".
pub fn tag_status(p: Progress) -> String {
    if p.excess() > 0 {
        format!("+{} de excedente", short(p.excess()))
    } else if p.is_finished() {
        "Concluído".into()
    } else {
        format!("Restam {}", short(p.remaining()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        assert_eq!(hms(5_760), "01:36:00");
        assert_eq!(hms(7_084), "01:58:04");
        assert_eq!(hms(-5), "00:00:00");
        assert_eq!(hms(100 * 3600), "100:00:00");
        assert_eq!(hm(5_760), "1h36min");
        assert_eq!(hm(2_880), "0h48min");
        assert_eq!(hm(7_084), "1h58min");
        assert_eq!(hm(57_600), "16h00min");
        assert_eq!(short(2_880), "48min");
        assert_eq!(short(7_200), "2h");
        assert_eq!(short(5_760), "1h36min");
    }

    #[test]
    fn datas_em_portugues() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(day_title(d), "Segunda-feira, 9 de novembro");
        assert_eq!(short_date(d), "Seg, 09/11");
        assert_eq!(short_date(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()), "Sáb, 14/11");
    }

    #[test]
    fn status_da_tag() {
        assert_eq!(tag_status(Progress::new(5760, 600)), "Restam 1h26min");
        assert_eq!(tag_status(Progress::new(600, 600)), "Concluído");
        assert_eq!(tag_status(Progress::new(600, 1320)), "+12min de excedente");
    }
}
