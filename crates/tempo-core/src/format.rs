//! Formatação de durações (RN08).

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
}
