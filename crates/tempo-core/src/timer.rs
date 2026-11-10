//! Cálculos do timer baseados em marcação de tempo (RN18).

/// Um trecho de tempo registrado. `end == None` significa timer rodando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: i64,
    pub end: Option<i64>,
}

impl Span {
    pub fn closed(start: i64, end: i64) -> Self {
        Self { start, end: Some(end) }
    }

    pub fn open(start: i64) -> Self {
        Self { start, end: None }
    }

    /// Duração até `now` (para sessões abertas). Nunca negativa (RN24).
    pub fn duration(&self, now: i64) -> i64 {
        (self.end.unwrap_or(now) - self.start).max(0)
    }

    /// Fim efetivo considerando `now` para sessões abertas.
    pub fn effective_end(&self, now: i64) -> i64 {
        self.end.unwrap_or(now).max(self.start)
    }

    /// Sobreposição estrita (encostar não conta).
    pub fn overlaps(&self, other: &Span, now: i64) -> bool {
        self.start < other.effective_end(now) && other.start < self.effective_end(now)
    }
}

/// Tempo consumido: soma de todas as sessões até `now`.
pub fn consumed(spans: &[Span], now: i64) -> i64 {
    spans.iter().map(|s| s.duration(now)).sum()
}

/// Situação de uma tag no dia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub allocated: i64,
    pub consumed: i64,
}

impl Progress {
    pub fn new(allocated: i64, consumed: i64) -> Self {
        Self { allocated: allocated.max(0), consumed: consumed.max(0) }
    }

    /// Tempo restante: `alocado − consumido`, mínimo 0.
    pub fn remaining(&self) -> i64 {
        (self.allocated - self.consumed).max(0)
    }

    /// Tempo além do alocado (RN15, RF21).
    pub fn excess(&self) -> i64 {
        (self.consumed - self.allocated).max(0)
    }

    pub fn is_finished(&self) -> bool {
        self.consumed >= self.allocated
    }

    /// Consumido ÷ alocado, limitado a 1 (RN29).
    pub fn ratio(&self) -> f32 {
        if self.allocated <= 0 {
            return if self.consumed > 0 { 1.0 } else { 0.0 };
        }
        (self.consumed as f64 / self.allocated as f64).min(1.0) as f32
    }
}

/// Instante em que o timer atinge o alocado, dado o início da sessão ativa e o consumido
/// antes dela. Se já estava esgotado, é o próprio início.
pub fn deadline(active_start: i64, consumed_before: i64, allocated: i64) -> i64 {
    active_start + (allocated - consumed_before).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumido_soma_sessoes_abertas_e_fechadas() {
        let spans = [Span::closed(0, 600), Span::closed(1000, 1300), Span::open(2000)];
        assert_eq!(consumed(&spans, 2100), 600 + 300 + 100);
    }

    #[test]
    fn duracao_nunca_negativa() {
        assert_eq!(Span::open(100).duration(50), 0);
        assert_eq!(Span::closed(100, 90).duration(0), 0);
    }

    #[test]
    fn progresso() {
        let p = Progress::new(5_760, 600);
        assert_eq!(p.remaining(), 5_160);
        assert_eq!(p.excess(), 0);
        assert!(!p.is_finished());
        let p = Progress::new(600, 750);
        assert_eq!(p.remaining(), 0);
        assert_eq!(p.excess(), 150);
        assert!(p.is_finished());
        assert_eq!(p.ratio(), 1.0);
        assert_eq!(Progress::new(0, 0).ratio(), 0.0);
        assert_eq!(Progress::new(0, 10).ratio(), 1.0);
        assert!((Progress::new(1000, 250).ratio() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn prazo_do_timer() {
        assert_eq!(deadline(1_000, 600, 5_760), 1_000 + 5_160);
        assert_eq!(deadline(1_000, 9_000, 5_760), 1_000);
    }

    #[test]
    fn sobreposicao() {
        let a = Span::closed(100, 200);
        assert!(a.overlaps(&Span::closed(150, 250), 0));
        assert!(!a.overlaps(&Span::closed(200, 300), 0)); // encostar não conta
        assert!(a.overlaps(&Span::open(50), 120));
        assert!(!a.overlaps(&Span::open(250), 300));
    }
}
