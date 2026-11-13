//! Aba "Agora": cronômetro de uma atividade, desenhado como um copo que enche até a marca da meta.
//!
//! Usa o mesmo timer da aba "Hoje" (uma sessão por tag); aqui o tempo é mostrado subindo.

use leptos::prelude::*;
use tempo_core::format::{goal_caption, hms, short};
use tempo_store::{TagToday, TodayView};
use wasm_bindgen::prelude::*;

use super::today::toggle_tag;
use crate::state::{ctx, Confirm, Page};
use crate::ui::{css, icon, Icon};

#[wasm_bindgen(module = "/src/pour.js")]
extern "C" {
    /// Jato de líquido da tag selecionada até o copo (lê o estado dos atributos `data-*` do copo).
    #[wasm_bindgen(js_name = startPour)]
    fn start_pour(canvas: &web_sys::HtmlCanvasElement);
}

// Geometria do copo (unidades do viewBox): fundo e borda do interior, e a altura da marca.
const BOTTOM: f32 = 246.0;
const RIM: f32 = 12.0;
/// Fração da altura interna em que fica a marca da meta; o resto é espaço para o excedente.
const MARK: f32 = 0.8;
const INSIDE: &str = "M27 12 L173 12 L152 238 Q151 246 143 246 L57 246 Q49 246 48 238 Z";
const OUTLINE: &str = "M26 6 L48 238 Q49 246 57 246 L143 246 Q151 246 152 238 L174 6";
/// Líquido parado: superfície reta (o movimento fica só no ponto onde o jato cai).
const LIQUID: &str = "M0 0 H200 V300 H0 Z";

fn mark_y() -> f32 {
    BOTTOM - MARK * (BOTTOM - RIM)
}

/// Altura (y) da superfície do líquido para um nível `fill` (1,0 = marca da meta).
fn level_y(fill: f32) -> f32 {
    if fill <= 0.0 {
        // Esconde também a crista da onda.
        return BOTTOM + 8.0;
    }
    (BOTTOM - fill * MARK * (BOTTOM - RIM)).max(RIM)
}

#[component]
pub fn NowPage() -> impl IntoView {
    let c = ctx();
    let today = Memo::new(move |_| {
        c.now.track();
        c.read(|t| t.today())
    });
    let tags = Memo::new(move |_| {
        today.with(|v| {
            v.as_ref()
                .map(|v| v.tags.iter().map(|t| (t.tag.id, t.tag.name.clone(), css(t.tag.color))).collect::<Vec<_>>())
                .unwrap_or_default()
        })
    });
    let has_tags = Memo::new(move |_| tags.with(|t| !t.is_empty()));

    // Atividade escolhida; sem escolha, acompanha o timer em andamento (ou a última usada).
    let picked = RwSignal::new(None::<i64>);
    let selected = Memo::new(move |_| {
        today.with(|v| {
            let v = v.as_ref()?;
            let exists = |id: &i64| v.tags.iter().any(|t| t.tag.id == *id);
            picked
                .get()
                .filter(exists)
                .or(v.active.as_ref().map(|a| a.tag_id))
                .or(v.focus_tag.filter(exists))
                .or(v.tags.first().map(|t| t.tag.id))
        })
    });
    let running_id = Memo::new(move |_| today.with(|v| v.as_ref().and_then(|v| v.active.as_ref().map(|a| a.tag_id))));
    let row = Memo::new(move |_| {
        let id = selected.get()?;
        today.with(|v| v.as_ref().and_then(|v: &TodayView| v.tags.iter().find(|t| t.tag.id == id).cloned()))
    });

    let s = move |f: fn(&TagToday) -> String| move || row.with(|r| r.as_ref().map(f).unwrap_or_default());
    let b = move |f: fn(&TagToday) -> bool| move || row.with(|r| r.as_ref().is_some_and(f));
    let running = b(|t| t.running);
    let reached = b(|t| t.allocated > 0 && t.progress().is_finished());
    let has_goal = b(|t| t.allocated > 0);
    let started = b(|t| t.consumed > 0);
    let color = s(|t| css(t.tag.color));
    let liquid_y = move || row.with(|r| level_y(r.as_ref().map_or(0.0, |t| t.progress().fill())));
    // O jato cai da tag até a superfície (ou até o fundo, com o copo vazio).
    let stream_end = move || format!("{:.2}", liquid_y().min(BOTTOM - 2.0));
    let empty = move || row.with(|r| r.as_ref().is_none_or(|t| t.progress().fill() <= 0.0));
    let pour = NodeRef::<leptos::html::Canvas>::new();
    Effect::new(move |_| {
        if let Some(canvas) = pour.get() {
            start_pour(&canvas);
        }
    });
    // Outra atividade com o cronômetro ligado enquanto esta está na tela.
    let other_running = move || {
        let other = running_id.get().filter(|id| Some(*id) != selected.get())?;
        tags.with(|t| t.iter().find(|(id, ..)| *id == other).map(|(_, name, _)| name.clone()))
    };

    view! {
        <header class="page-head">
            <div class="grow">
                <h1>"Agora"</h1>
                <p class="muted">"Cronometre uma atividade e encha o copo até a marca da meta."</p>
            </div>
        </header>

        <Show
            when=move || has_tags.get()
            fallback=move || view! {
                <Show when=move || today.with(Option::is_some)>
                    <section class="card">
                        <h2>"Nenhuma atividade planejada"</h2>
                        <p class="muted">"Crie suas tags em Planejar para cronometrar o tempo de cada uma."</p>
                        <button type="button" class="btn primary" on:click=move |_| c.page.set(Page::Plan)>
                            <Icon path=icon::TUNE />
                            "Planejar tags"
                        </button>
                    </section>
                </Show>
            }
        >
            <section class="card now-card">
                <canvas class="pour" node_ref=pour aria-hidden="true"></canvas>
                <div class="now-tags" role="radiogroup" aria-label="Atividade">
                    <For each=move || tags.get() key=|t| t.clone() let:t>
                        {
                            let (id, name, tag_color) = t;
                            view! {
                                <button
                                    type="button"
                                    class="now-tag"
                                    class:active=move || selected.get() == Some(id)
                                    class:running=move || running_id.get() == Some(id)
                                    role="radio"
                                    aria-checked=move || (selected.get() == Some(id)).to_string()
                                    on:click=move |_| picked.set(Some(id))
                                >
                                    <span class="dot" style:background=tag_color></span>
                                    <span class="ellipsis">{name}</span>
                                </button>
                            }
                        }
                    </For>
                </div>

                <svg
                    class="glass"
                    class:running=running
                    class:reached=reached
                    viewBox="-60 0 320 260"
                    data-pour=move || if running() { "1" } else { "0" }
                    data-color=color
                    data-surface=stream_end
                    data-empty=move || if empty() { "1" } else { "0" }
                    role="img"
                    aria-label=move || {
                        row.with(|r| {
                            r.as_ref()
                                .map(|t| {
                                    format!(
                                        "{}: {} cronometrados. {}",
                                        t.tag.name,
                                        short(t.consumed),
                                        goal_caption(t.progress())
                                    )
                                })
                                .unwrap_or_default()
                        })
                    }
                >
                    <defs>
                        <clipPath id="glass-inside">
                            <path d=INSIDE />
                        </clipPath>
                    </defs>
                    <path class="glass-back" d=INSIDE />
                    <g clip-path="url(#glass-inside)">
                        <g class="glass-liquid" style:transform=move || format!("translateY({:.2}px)", liquid_y())>
                            <path class="glass-wave" d=LIQUID style:fill=color />
                            <line class="glass-surface" x1="0" x2="200" y1="0" y2="0" />
                        </g>
                    </g>
                    <path class="glass-outline" d=OUTLINE />
                    <Show when=has_goal>
                        <line class="glass-mark" x1="33" x2="167" y1=mark_y() y2=mark_y() />
                        <line class="glass-tick" x1="150" x2="188" y1=mark_y() y2=mark_y() />
                        <text class="glass-mark-label" x="194" y=mark_y() - 4.0>"META"</text>
                        <text class="glass-mark-value" x="194" y=mark_y() + 15.0>{s(|t| short(t.allocated))}</text>
                        <text class="glass-mark-pct" x="194" y=mark_y() + 31.0>{s(|t| t.permille.to_string())}</text>
                    </Show>
                </svg>

                <div class="hero-time" role="timer" aria-live="off">{s(|t| hms(t.consumed))}</div>
                <p class="center strong" class:ok=reached class:muted=move || !reached()>
                    {s(|t| goal_caption(t.progress()))}
                </p>

                <div class="row center gap">
                    <button
                        type="button"
                        class="btn primary"
                        on:click=move |_| {
                            if let Some(id) = selected.get_untracked() {
                                picked.set(Some(id));
                                toggle_tag(c, id);
                            }
                        }
                    >
                        {move || view! { <Icon path=if running() { icon::PAUSE } else { icon::PLAY } /> }}
                        {move || {
                            if running() {
                                "Pausar"
                            } else if started() {
                                "Continuar"
                            } else {
                                "Iniciar cronômetro"
                            }
                        }}
                    </button>
                    <Show when=started>
                        <button
                            type="button"
                            class="btn"
                            on:click=move |_| {
                                let Some(t) = row.get_untracked() else { return };
                                c.confirm(
                                    format!("Apagar o progresso de \"{}\"?", t.tag.name),
                                    format!(
                                        "O tempo registrado hoje nesta atividade ({}) será apagado e o copo volta a ficar vazio. Isso não pode ser desfeito.",
                                        hms(t.consumed)
                                    ),
                                    "Apagar",
                                    Confirm::ClearTagToday(t.tag.id),
                                );
                            }
                        >
                            <Icon path=icon::DELETE />
                            "Apagar progresso"
                        </button>
                    </Show>
                </div>
                {move || {
                    other_running().map(|name| view! {
                        <p class="muted small center">
                            {format!("\"{name}\" está sendo cronometrada. Iniciar aqui pausa ela.")}
                        </p>
                    })
                }}
            </section>
        </Show>
    }
}
