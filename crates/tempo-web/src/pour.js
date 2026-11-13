// Aba "Agora": jato de líquido saindo da tag selecionada (a torneira) até o copo.
//
// Desenhado num <canvas> sobre o cartão. O Rust só informa o estado no <svg class="glass">:
//   data-pour="1"    o cronômetro da atividade mostrada está rodando
//   data-color       cor do líquido
//   data-surface     altura da superfície, em unidades do desenho do copo
//   data-empty="1"   copo vazio (o jato bate no fundo)
//
// Física: cada trecho do jato é uma "parcela" de líquido com idade `a` desde que saiu do bico.
// Vertical: gravidade. Horizontal: velocidade inicial com arrasto (vai de lado logo na saída e
// depois cai quase reto), calculada para o jato passar pela boca do copo. A espessura segue a
// continuidade (afina conforme acelera, sem passar de metade da espessura da saída).

// Geometria do copo, nas unidades do viewBox "-60 0 320 260".
const VB_X = -60;
const VB_W = 320;
const RIM = 12;
const CX = 100;
const TOP_HALF = 73; // meia largura interna na borda (27..173)
const SLOPE = 21 / 226; // quanto a parede entra por unidade de altura
const INSIDE = [
  [27, 12],
  [173, 12],
  [152, 238],
  [148, 245],
  [52, 245],
  [48, 238],
];

const NOZZLE = 6; // altura do bico desenhado em CSS sob a tag (px)
const RIPPLE = 0.09; // comprimento das ondulações que descem pelo fio (s)
const SLOW = 0.5; // câmera lenta: o líquido cai na metade da velocidade física

const reduce = matchMedia("(prefers-reduced-motion: reduce)");
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const smooth = (e0, e1, x) => {
  const t = clamp((x - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
};

/** Liga o jato no canvas (filho do cartão). Para sozinho quando o canvas sai da página. */
export function startPour(canvas) {
  if (canvas.__pour) return;
  canvas.__pour = true;

  const card = canvas.parentElement;
  const ctx = canvas.getContext("2d");
  let raf = 0;
  let last = 0;
  let now = 0;
  let wasOn = false;
  let surf = null;
  let splashAcc = 0;
  /** Trechos do jato: o atual (stopped = null) e os que se soltaram ao pausar. */
  let segs = [];
  let splashes = [];

  const kick = () => {
    if (!raf) raf = requestAnimationFrame(frame);
  };
  const observer = new MutationObserver(kick);
  observer.observe(card, {
    subtree: true,
    attributes: true,
    attributeFilter: ["data-pour", "data-surface", "class"],
  });
  window.addEventListener("resize", kick);

  function frame(ts) {
    raf = 0;
    if (!canvas.isConnected) {
      observer.disconnect();
      window.removeEventListener("resize", kick);
      canvas.__pour = false;
      return;
    }
    const realDt = last ? Math.min(0.05, (ts - last) / 1000) : 1 / 60;
    last = ts;
    // O jato roda em câmera lenta (mesma trajetória, metade da velocidade).
    const dt = realDt * SLOW;
    now += dt;

    // Canvas do tamanho do cartão, nítido em telas de alta densidade.
    const cr = card.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(cr.width * dpr);
    const H = Math.round(cr.height * dpr);
    if (canvas.width !== W || canvas.height !== H) {
      canvas.width = W;
      canvas.height = H;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cr.width, cr.height);

    const glass = card.querySelector("svg.glass");
    if (!glass) return idle();
    const chip = card.querySelector(".now-tag.active");
    const gr = glass.getBoundingClientRect();
    const u = gr.width / VB_W; // px por unidade do desenho
    const gx = gr.left - cr.left - VB_X * u;
    const gy = gr.top - cr.top;
    const px = (x) => gx + x * u;
    const py = (y) => gy + y * u;

    const on = glass.dataset.pour === "1" && !!chip;
    const color = glass.dataset.color || "#888";
    const empty = glass.dataset.empty === "1";
    const target = parseFloat(glass.dataset.surface) || 244;
    // Mesmo ritmo da transição CSS que sobe o líquido.
    surf = surf === null || reduce.matches ? target : surf + (target - surf) * Math.min(1, realDt * 2.5);
    const sy = py(surf);

    const g = 1900 * u;
    const v0 = 60 * u; // velocidade de saída, para baixo
    const w0 = 7.5 * u; // espessura na saída

    if (on && !wasOn) segs.push({ born: now, stopped: null, ox: 0, oy: 0, vx: 0, k: 2 });
    if (!on && wasOn) segs.forEach((s) => s.stopped ??= now);
    wasOn = on;

    const fallTime = (s, y) => {
      const d = Math.max(0.5, y - s.oy);
      return (-v0 + Math.sqrt(v0 * v0 + 2 * g * d)) / g;
    };
    const pos = (s, a) => [s.ox + (s.vx / s.k) * (1 - Math.exp(-s.k * a)), s.oy + v0 * a + 0.5 * g * a * a];
    const vel = (s, a) => [s.vx * Math.exp(-s.k * a), v0 + g * a];
    const width = (a) => w0 * Math.max(0.55, Math.sqrt((200 * u) / (200 * u + g * a)));

    // O trecho ligado acompanha a tag (o layout pode mudar) e mira na boca do copo.
    const live = segs.find((s) => s.stopped === null);
    if (live && chip) {
      const r = chip.getBoundingClientRect();
      live.ox = r.left - cr.left + r.width / 2;
      live.oy = r.bottom - cr.top + NOZZLE - 1;
      aim(live, (live.ox - gx) / u);
    }

    function aim(s, ox) {
      const tR = fallTime(s, py(RIM));
      const tS = fallTime(s, sy);
      // Cai perto do centro: com a parede longe, o jato não cruza o vidro nem a marca da meta.
      const half = (TOP_HALF - (surf - RIM) * SLOPE) * 0.45;
      const land = clamp(ox, CX - half, CX + half);
      const rimHalf = TOP_HALF - 26;
      // Fração do deslocamento lateral já feita ao cruzar a borda, para um arrasto k.
      const frac = (k) => (1 - Math.exp(-k * tR)) / (1 - Math.exp(-k * tS));
      const atRim = (k) => ox + (land - ox) * frac(k);
      let k = 2;
      if (tR < tS && Math.abs(atRim(k) - CX) > rimHalf) {
        // Mais arrasto = curva mais cedo; busca o mínimo que passa pela boca.
        let lo = 2;
        let hi = 80;
        for (let i = 0; i < 24; i++) {
          const mid = (lo + hi) / 2;
          if (Math.abs(atRim(mid) - CX) > rimHalf) lo = mid;
          else hi = mid;
        }
        k = hi;
      }
      s.k = k;
      s.vx = ((land - ox) * u * k) / (1 - Math.exp(-k * tS));
    }

    let impact = null;
    ctx.save();
    ctx.globalAlpha = 0.92;
    ctx.fillStyle = color;
    segs = segs.filter((s) => {
      const aEnd = fallTime(s, sy);
      const head = reduce.matches ? aEnd : Math.min(now - s.born, aEnd);
      const tail = s.stopped === null ? 0 : reduce.matches ? aEnd : now - s.stopped;
      if (tail >= aEnd) return false;
      if (head >= aEnd) impact = { x: pos(s, aEnd)[0], w: width(aEnd) };
      drawStream(s, tail, head, aEnd);
      return true;
    });
    ctx.restore();

    function drawStream(s, tail, head, aEnd) {
      const still = reduce.matches;
      const ribbonEnd = head;

      // Ondulação suave e igual em todo o fio, viajando junto com o líquido.
      const neck = (a) => {
        if (still) return 1;
        const e = now - a; // momento em que esta parcela saiu do bico
        return 1 + 0.07 * Math.sin((2 * Math.PI * e) / RIPPLE);
      };
      // Balanço lateral leve do fio.
      const sway = (a) => (still ? 0 : w0 * 0.15 * smooth(0, 0.15, a) * Math.sin(now * 7 - a * 18));

      if (ribbonEnd > tail) {
        const n = 56;
        const left = [];
        const right = [];
        const shine = [];
        for (let i = 0; i <= n; i++) {
          const a = tail + ((ribbonEnd - tail) * i) / n;
          const [x, y] = pos(s, a);
          const [vx, vy] = vel(s, a);
          const len = Math.hypot(vx, vy) || 1;
          const nx = -vy / len;
          const ny = vx / len;
          const hw = (width(a) * neck(a)) / 2;
          const off = sway(a);
          const cx = x + nx * off;
          const cy = y + ny * off;
          left.push([cx + nx * hw, cy + ny * hw]);
          right.push([cx - nx * hw, cy - ny * hw]);
          shine.push([cx + nx * hw * 0.45, cy + ny * hw * 0.45, hw]);
        }
        ctx.beginPath();
        ctx.moveTo(left[0][0], left[0][1]);
        for (const p of left) ctx.lineTo(p[0], p[1]);
        for (let i = right.length - 1; i >= 0; i--) ctx.lineTo(right[i][0], right[i][1]);
        ctx.closePath();
        ctx.fill();

        // Pontas arredondadas: a gota da frente, o fim que se soltou e a saída da torneira.
        const cap = (a, grow) => {
          const [x, y] = pos(s, a);
          const [vx, vy] = vel(s, a);
          const len = Math.hypot(vx, vy) || 1;
          const off = sway(a);
          const r = (width(a) * neck(a) * grow) / 2;
          ctx.beginPath();
          ctx.arc(x - (vy / len) * off, y + (vx / len) * off, r, 0, Math.PI * 2);
          ctx.fill();
        };
        if (head < aEnd) cap(head, 1.35);
        if (tail > 0) cap(tail, 1.15);
        else {
          ctx.beginPath();
          ctx.ellipse(s.ox, s.oy + w0 * 0.1, w0 * 0.62, w0 * 0.4, 0, 0, Math.PI * 2);
          ctx.fill();
        }

        // Brilho: um reflexo fino do lado da luz dá volume ao fio.
        ctx.save();
        ctx.globalAlpha = 0.32;
        ctx.strokeStyle = "#fff";
        ctx.lineCap = "round";
        ctx.lineWidth = Math.max(1, w0 * 0.16);
        ctx.beginPath();
        const skip = Math.ceil(shine.length * 0.04);
        for (let i = skip; i < shine.length - skip; i++) {
          if (i === skip) ctx.moveTo(shine[i][0], shine[i][1]);
          else ctx.lineTo(shine[i][0], shine[i][1]);
        }
        ctx.stroke();
        ctx.restore();
      }

    }

    // Onde o jato bate: respingos e o volume que ele empurra na superfície.
    const still = reduce.matches;
    if (impact && !still) {
      splashAcc += dt * 46;
      while (splashAcc >= 1) {
        splashAcc -= 1;
        const side = Math.random() < 0.5 ? -1 : 1;
        splashes.push({
          x: impact.x + side * impact.w * 0.35,
          y: sy - u,
          vx: side * (25 + Math.random() * 110) * u,
          vy: -(70 + Math.random() * 190) * u,
          r: (0.7 + Math.random() * 1.5) * u,
        });
      }
    }
    for (const p of splashes) {
      p.vy += g * 0.85 * dt;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
    }
    splashes = splashes.filter((p) => !(p.vy > 0 && p.y > sy));

    if (impact || splashes.length) {
      ctx.save();
      ctx.beginPath();
      INSIDE.forEach(([x, y], i) => (i ? ctx.lineTo(px(x), py(y)) : ctx.moveTo(px(x), py(y))));
      ctx.closePath();
      ctx.clip();

      if (impact && !empty) {
        // Monte que o jato levanta onde cai: sobe e desce no lugar, sem correnteza para os lados.
        const spread = impact.w * 1.4 + 4 * u;
        const lift = (2.6 + (still ? 0 : 0.8 * Math.sin(now * 40))) * u;
        const base = sy + 0.5 * u;
        ctx.globalAlpha = 0.9;
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.moveTo(impact.x - spread * 2.5, base);
        for (let i = 0; i <= 40; i++) {
          const dx = spread * 2.5 * (i / 20 - 1);
          ctx.lineTo(impact.x + dx, base - lift * Math.exp(-((dx / spread) ** 2)));
        }
        ctx.closePath();
        ctx.fill();
      }

      ctx.fillStyle = color;
      for (const p of splashes) {
        ctx.globalAlpha = 0.9;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    }

    if (on || segs.length || splashes.length) raf = requestAnimationFrame(frame);
    else idle();
  }

  function idle() {
    last = 0;
    surf = null;
  }

  kick();
}
