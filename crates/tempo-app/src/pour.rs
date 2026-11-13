//! Jato da aba "Agora": a mesma física do `pour.js` da versão web, devolvida como caminhos SVG
//! para o Slint desenhar.
//!
//! Cada trecho do jato é uma "parcela" de líquido com idade `a` desde que saiu do bico.
//! Vertical: gravidade. Horizontal: velocidade inicial com arrasto (vai de lado logo na saída e
//! depois cai quase reto), calculada para o jato passar pela boca do copo. A espessura segue a
//! continuidade (afina conforme acelera, sem passar de metade da espessura da saída).
//!
//! Tudo em unidades do desenho do copo (quadro de 320×260, centro em x = 160) e sem estado:
//! o quadro depende só dos argumentos, então o Slint pode pedir um novo a cada frame.

use std::f32::consts::TAU;
use std::fmt::Write;

const G: f32 = 1900.0;
/// Velocidade de saída, para baixo.
const V0: f32 = 60.0;
/// Espessura na saída.
const W0: f32 = 7.5;
const RIM: f32 = 12.0;
const CX: f32 = 160.0;
/// Meia largura interna na borda.
const TOP_HALF: f32 = 73.0;
/// Quanto a parede entra por unidade de altura.
const SLOPE: f32 = 21.0 / 226.0;
/// Comprimento das ondulações que descem pelo fio (s).
const RIPPLE: f32 = 0.09;
/// Pontos ao longo do fio.
const STEPS: usize = 48;
/// Respingos: vagas que se revezam, e quantos nascem por segundo.
const DROPS: usize = 16;
const DROP_RATE: f32 = 46.0;

/// Caminhos SVG de um quadro do jato.
#[derive(Default, Debug)]
pub struct Frame {
    /// Fio de líquido (preenchido).
    pub stream: String,
    /// Pontas arredondadas: a gota da frente e o fim que se soltou.
    pub caps: String,
    /// Reflexo fino do lado da luz (traço).
    pub shine: String,
    /// Monte que o jato levanta onde cai.
    pub mound: String,
    /// Respingos.
    pub drops: String,
}

struct Jet {
    ox: f32,
    oy: f32,
    vx: f32,
    k: f32,
}

impl Jet {
    /// Mira na boca do copo: cai perto do centro, sem cruzar o vidro nem a marca da meta.
    fn aim(ox: f32, oy: f32, sy: f32) -> Self {
        let mut jet = Jet { ox, oy, vx: 0.0, k: 2.0 };
        let t_rim = jet.fall_time(RIM);
        let t_surf = jet.fall_time(sy);
        let half = (TOP_HALF - (sy - RIM) * SLOPE) * 0.45;
        let land = ox.clamp(CX - half, CX + half);
        let rim_half = TOP_HALF - 26.0;
        // Posição ao cruzar a borda, para um arrasto k.
        let at_rim = |k: f32| ox + (land - ox) * (1.0 - (-k * t_rim).exp()) / (1.0 - (-k * t_surf).exp());
        if t_rim < t_surf && (at_rim(jet.k) - CX).abs() > rim_half {
            // Mais arrasto = curva mais cedo; busca o mínimo que passa pela boca.
            let (mut lo, mut hi) = (2.0_f32, 80.0_f32);
            for _ in 0..24 {
                let mid = (lo + hi) / 2.0;
                if (at_rim(mid) - CX).abs() > rim_half {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            jet.k = hi;
        }
        jet.vx = (land - ox) * jet.k / (1.0 - (-jet.k * t_surf).exp());
        jet
    }

    fn fall_time(&self, y: f32) -> f32 {
        let d = (y - self.oy).max(0.5);
        (-V0 + (V0 * V0 + 2.0 * G * d).sqrt()) / G
    }

    fn pos(&self, a: f32) -> (f32, f32) {
        (self.ox + self.vx / self.k * (1.0 - (-self.k * a).exp()), self.oy + V0 * a + 0.5 * G * a * a)
    }

    /// Direção do movimento (unitária).
    fn dir(&self, a: f32) -> (f32, f32) {
        let (vx, vy) = (self.vx * (-self.k * a).exp(), V0 + G * a);
        let len = vx.hypot(vy).max(1e-3);
        (vx / len, vy / len)
    }
}

fn width(a: f32) -> f32 {
    W0 * (200.0 / (200.0 + G * a)).sqrt().max(0.55)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Número pseudoaleatório em [0, 1), fixo para cada (vaga, geração, canal).
fn rand(slot: u32, generation: u32, channel: u32) -> f32 {
    let mut h =
        slot.wrapping_mul(0x9E37_79B1) ^ generation.wrapping_mul(0x85EB_CA77) ^ channel.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn circle(out: &mut String, x: f32, y: f32, rx: f32, ry: f32) {
    let _ = write!(
        out,
        "M{:.2} {:.2}a{rx:.2} {ry:.2} 0 1 0 {:.2} 0a{rx:.2} {ry:.2} 0 1 0 {:.2} 0Z",
        x - rx,
        y,
        2.0 * rx,
        -2.0 * rx
    );
}

/// Um quadro do jato.
///
/// - `fx`, `fy`: bico da torneira; `sy`: altura da superfície (ou do fundo, com o copo `empty`).
/// - `head`: há quanto tempo o jato foi ligado; `tail`: há quanto tempo foi desligado (0 = ligado).
/// - `now`: relógio da animação. Todos os tempos em segundos do jato (já em câmera lenta).
pub fn frame(fx: f32, fy: f32, sy: f32, empty: bool, head: f32, tail: f32, now: f32) -> Frame {
    let mut out = Frame::default();
    let jet = Jet::aim(fx, fy, sy);
    let a_end = jet.fall_time(sy);
    let impact = head >= a_end && tail < a_end;
    let (land_x, land_w) = (jet.pos(a_end).0, width(a_end));

    if tail < a_end && head > tail {
        let front = head.min(a_end);
        // Ondulação suave viajando junto com o líquido, e um balanço lateral leve do fio.
        let neck = |a: f32| 1.0 + 0.07 * (TAU * (now - a) / RIPPLE).sin();
        let sway = |a: f32| W0 * 0.15 * smooth(0.0, 0.15, a) * (now * 7.0 - a * 18.0).sin();
        // Centro do fio na idade `a`, a normal e a meia espessura.
        let at = |a: f32| {
            let (x, y) = jet.pos(a);
            let (dx, dy) = jet.dir(a);
            let (nx, ny) = (-dy, dx);
            let off = sway(a);
            (x + nx * off, y + ny * off, nx, ny, width(a) * neck(a) / 2.0)
        };

        let points: Vec<_> = (0..=STEPS).map(|i| at(tail + (front - tail) * i as f32 / STEPS as f32)).collect();
        for (i, (x, y, nx, ny, hw)) in points.iter().enumerate() {
            let _ = write!(out.stream, "{}{:.2} {:.2}", if i == 0 { 'M' } else { 'L' }, x + nx * hw, y + ny * hw);
        }
        for (x, y, nx, ny, hw) in points.iter().rev() {
            let _ = write!(out.stream, "L{:.2} {:.2}", x - nx * hw, y - ny * hw);
        }
        out.stream.push('Z');

        let skip = (points.len() as f32 * 0.04).ceil() as usize;
        for (i, (x, y, nx, ny, hw)) in points[skip..points.len() - skip].iter().enumerate() {
            let _ = write!(
                out.shine,
                "{}{:.2} {:.2}",
                if i == 0 { 'M' } else { 'L' },
                x + nx * hw * 0.45,
                y + ny * hw * 0.45
            );
        }

        let mut cap = |a: f32, grow: f32| {
            let (x, y, _, _, hw) = at(a);
            circle(&mut out.caps, x, y, hw * grow, hw * grow);
        };
        if head < a_end {
            cap(head, 1.35);
        }
        // Ligado, o fio sai reto de dentro do tubo da torneira (sem ponta arredondada).
        if tail > 0.0 {
            cap(tail, 1.15);
        }
    }

    if impact && !empty {
        // Sobe e desce no lugar, sem correnteza para os lados.
        let spread = land_w * 1.4 + 4.0;
        let lift = 2.6 + 0.8 * (now * 40.0).sin();
        let base = sy + 0.5;
        let _ = write!(out.mound, "M{:.2} {:.2}", land_x - spread * 2.5, base);
        for i in 0..=40 {
            let dx = spread * 2.5 * (i as f32 / 20.0 - 1.0);
            let _ = write!(out.mound, "L{:.2} {:.2}", land_x + dx, base - lift * (-(dx / spread).powi(2)).exp());
        }
        out.mound.push('Z');
    }

    // Respingos: cada vaga solta uma gota por ciclo; só valem as que nasceram com o jato batendo.
    let cycle = DROPS as f32 / DROP_RATE;
    for slot in 0..DROPS {
        let t = now - slot as f32 * cycle / DROPS as f32;
        let generation = (t / cycle).floor();
        let age = t - generation * cycle;
        if age > head - a_end || age < tail - a_end {
            continue;
        }
        let r = |channel| rand(slot as u32, generation as i64 as u32, channel);
        let side = if r(0) < 0.5 { -1.0 } else { 1.0 };
        let vx = side * (25.0 + r(1) * 110.0);
        let vy = -(70.0 + r(2) * 190.0);
        let radius = 0.7 + r(3) * 1.5;
        let x = land_x + side * land_w * 0.35 + vx * age;
        let y = sy - 1.0 + vy * age + 0.5 * G * 0.85 * age * age;
        // Some ao voltar para a superfície; fora do vidro não aparece.
        let inside = (x - CX).abs() + radius < TOP_HALF - (y - RIM) * SLOPE && y > RIM;
        if y <= sy && inside {
            circle(&mut out.drops, x, y, radius, radius);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Torneira à esquerda e acima do copo, com outra linha de tags no caminho.
    const FX: f32 = 45.0;
    const FY: f32 = -110.0;

    #[test]
    fn jato_passa_pela_boca_e_cai_perto_do_centro() {
        for sy in [40.0, 120.0, 244.0] {
            let jet = Jet::aim(FX, FY, sy);
            let (x_rim, _) = jet.pos(jet.fall_time(RIM));
            let (x, y) = jet.pos(jet.fall_time(sy));
            assert!((x_rim - CX).abs() <= TOP_HALF - 26.0 + 0.1, "na borda: {x_rim}");
            assert!((y - sy).abs() < 0.1);
            assert!((x - CX).abs() < TOP_HALF * 0.45 + 0.1, "na superfície: {x}");
        }
    }

    #[test]
    fn desligado_nao_desenha_nada() {
        let f = frame(FX, FY, 120.0, false, 0.0, 2.0, 5.0);
        assert!(f.stream.is_empty() && f.caps.is_empty() && f.mound.is_empty() && f.drops.is_empty());
    }

    #[test]
    fn caindo_ainda_nao_bate_na_superficie() {
        let f = frame(FX, FY, 120.0, false, 0.1, 0.0, 5.0);
        assert!(f.stream.starts_with('M') && f.stream.ends_with('Z'));
        assert!(f.mound.is_empty() && f.drops.is_empty());
    }

    #[test]
    fn ligado_levanta_o_monte_e_respinga() {
        let f = frame(FX, FY, 120.0, false, 2.0, 0.0, 5.0);
        assert!(!f.stream.is_empty() && !f.shine.is_empty() && !f.mound.is_empty() && !f.drops.is_empty());
        // Copo vazio: o jato bate no fundo, sem monte.
        assert!(frame(FX, FY, 244.0, true, 2.0, 0.0, 5.0).mound.is_empty());
    }

    #[test]
    fn ao_pausar_o_fim_do_jato_cai_e_some() {
        assert!(!frame(FX, FY, 120.0, false, 2.0, 0.1, 5.0).stream.is_empty());
        assert!(frame(FX, FY, 120.0, false, 2.0, 1.9, 5.0).stream.is_empty());
    }
}
