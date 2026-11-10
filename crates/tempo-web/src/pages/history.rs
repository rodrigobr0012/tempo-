//! Histórico por dia, estatísticas (RF30, RF31) e período com calendário pintável.

use std::cell::Cell;
use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use leptos::ev::PointerEvent;
use leptos::prelude::*;
use tempo_core::format::{short, short_date};
use tempo_core::Progress;
use tempo_store::{DaySummary, StatsView};

use crate::state::{ctx, HistoryTab};
use crate::ui::{css, Bar};

thread_local! {
    /// Estado da pintura de dias: `Some(target)` enquanto o botão do mouse está apertado,
    /// `None` quando solto. Guardamos o estado-alvo pra ficar coerente ao arrastar.
    static PAINTING: Cell<Option<bool>> = const { Cell::new(None) };
}

#[component]
pub fn HistoryPage() -> impl IntoView {
    let c = ctx();
    let tab = c.history_tab;
    let days = Memo::new(move |_| {
        if tab.get() != HistoryTab::Days {
            return Vec::new();
        }
        c.read(|t| {
            t.tick()?;
            t.history(60)
        })
        .unwrap_or_default()
    });
    let stats = Memo::new(move |_| {
        let period = match tab.get() {
            HistoryTab::Week => 7,
            HistoryTab::Month => 30,
            _ => return StatsView::default(),
        };
        c.read(|t| {
            t.tick()?;
            t.stats(period)
        })
        .unwrap_or_default()
    });

    let tab_btn = move |value: HistoryTab, label: &'static str| {
        view! {
            <button
                type="button"
                role="tab"
                class="seg"
                class:active=move || tab.get() == value
                aria-selected=move || (tab.get() == value).to_string()
                on:click=move |_| tab.set(value)
            >
                {label}
            </button>
        }
    };

    view! {
        <header class="page-head">
            <h1 class="grow">"Histórico"</h1>
        </header>
        <div class="segmented" role="tablist">
            {tab_btn(HistoryTab::Days, "Dias")}
            {tab_btn(HistoryTab::Week, "7 dias")}
            {tab_btn(HistoryTab::Month, "30 dias")}
            {tab_btn(HistoryTab::Period, "Período")}
        </div>

        {move || match tab.get() {
            HistoryTab::Days => view! {
                <Show when=move || days.with(Vec::is_empty)>
                    <p class="muted center">"Nada registrado ainda."</p>
                </Show>
                <div class="cards-grid">
                    {move || days.with(|d| d.iter().map(day_card).collect_view())}
                </div>
            }
            .into_any(),
            HistoryTab::Period => view! { <PeriodTab /> }.into_any(),
            _ => view! {
                <p class="muted">
                    {move || match stats.with(|s| s.days) {
                        0 => "Ainda não há dias completos neste período. As estatísticas aparecem a partir de amanhã.".to_string(),
                        1 => "Baseado em 1 dia completo.".to_string(),
                        n => format!("Baseado em {n} dias completos."),
                    }}
                </p>
                <div class="cards-grid">
                    {move || stats.with(|s| {
                        s.tags
                            .iter()
                            .map(|s| {
                                let st = &s.stats;
                                let days = if st.days_planned == 1 {
                                    "1 dia".to_string()
                                } else {
                                    format!("{} dias", st.days_planned)
                                };
                                view! {
                                    <section class="card">
                                        <div class="row">
                                            <span class="dot" style:background=css(s.color)></span>
                                            <h2 class="grow ellipsis">
                                                {format!("{}{}", s.name, if s.archived { " (arquivada)" } else { "" })}
                                            </h2>
                                            <span class="pct">{format!("{}%", (st.compliance * 100.0).round() as i32)}</span>
                                        </div>
                                        <Bar progress=st.compliance color=css(s.color) />
                                        <p class="muted small">
                                            {format!(
                                                "Total {} · média {}/dia · planejada em {}",
                                                short(st.consumed_total),
                                                short(st.daily_average),
                                                days
                                            )}
                                        </p>
                                    </section>
                                }
                            })
                            .collect_view()
                    })}
                </div>
            }
            .into_any(),
        }}
    }
}

fn day_card(d: &DaySummary) -> impl IntoView {
    let subtitle = if d.no_record {
        "Sem registro".to_string()
    } else {
        format!("{} registrados de {} disponíveis", short(d.consumed_total()), short(d.available))
    };
    let tags = d
        .tags
        .iter()
        .filter(|t| !d.no_record && (t.allocated > 0 || t.consumed > 0))
        .map(|t| {
            let p = Progress::new(t.allocated, t.consumed);
            view! {
                <div class="history-tag">
                    <div class="row small">
                        <span class="dot small-dot" style:background=css(t.color)></span>
                        <span class="grow ellipsis">{t.name.clone()}</span>
                        <span class="muted" class:danger={p.excess() > 0}>
                            {format!("{} / {}", short(t.consumed), short(t.allocated))}
                        </span>
                    </div>
                    <Bar progress=p.ratio() color=css(t.color) />
                </div>
            }
        })
        .collect_view();
    view! {
        <section class="card">
            <div class="row">
                <h2 class="grow">{short_date(d.date)}</h2>
                {(!d.closed).then(|| view! { <span class="chip">"Hoje"</span> })}
            </div>
            <p class="muted small">{subtitle}</p>
            {tags}
        </section>
    }
}

/// Soma por tag num conjunto de dias.
#[derive(Clone, PartialEq, Eq)]
struct UsageSlice {
    name: String,
    color: u32,
    consumed: i64,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Usage {
    /// Soma do tempo disponível (24h − reservado) dos dias escolhidos.
    available: i64,
    /// Soma por tag, do mais usado para o menos usado.
    tags: Vec<UsageSlice>,
}

impl Usage {
    fn consumed(&self) -> i64 {
        self.tags.iter().map(|t| t.consumed).sum()
    }
}

fn aggregate_days(summaries: &[DaySummary], selected: &std::collections::BTreeSet<NaiveDate>) -> Usage {
    let mut available = 0i64;
    let mut per_tag: BTreeMap<i64, UsageSlice> = BTreeMap::new();
    for d in summaries {
        if !selected.contains(&d.date) {
            continue;
        }
        available += d.available;
        for t in &d.tags {
            if t.consumed == 0 {
                continue;
            }
            let entry = per_tag.entry(t.tag_id).or_insert_with(|| UsageSlice {
                name: t.name.clone(),
                color: t.color,
                consumed: 0,
            });
            entry.consumed += t.consumed;
        }
    }
    let mut tags: Vec<UsageSlice> = per_tag.into_values().collect();
    tags.sort_by(|a, b| b.consumed.cmp(&a.consumed));
    Usage { available, tags }
}

#[component]
fn PeriodTab() -> impl IntoView {
    let c = ctx();
    let today = Memo::new(move |_| {
        c.now.track();
        chrono::Local::now().date_naive()
    });
    let summaries = Memo::new(move |_| {
        let month = c.calendar_month.get();
        c.rev.track();
        let (from, to) = month_bounds(month);
        c.read(|t| t.history_range(from, to)).unwrap_or_default()
    });
    let usage = Memo::new(move |_| {
        let sel = c.calendar_selected.get();
        summaries.with(|s| aggregate_days(s, &sel))
    });

    let shift_month = move |delta: i32| {
        c.calendar_month.update(|m| *m = shift_months(*m, delta));
    };

    view! {
        <section class="card period-card">
            <div class="calendar-head">
                <button
                    type="button"
                    class="btn text"
                    aria-label="Mês anterior"
                    on:click=move |_| shift_month(-1)
                >"◀"</button>
                <strong class="grow center">{move || month_title(c.calendar_month.get())}</strong>
                <button
                    type="button"
                    class="btn text"
                    aria-label="Próximo mês"
                    on:click=move |_| shift_month(1)
                >"▶"</button>
            </div>

            <CalendarGrid today=today />

            <div class="row small">
                <span class="grow muted">
                    {move || {
                        let n = c.calendar_selected.with(|s| s.len());
                        match n {
                            0 => "Nenhum dia selecionado. Clique ou arraste para pintar.".to_string(),
                            1 => "1 dia selecionado.".to_string(),
                            _ => format!("{n} dias selecionados."),
                        }
                    }}
                </span>
                <button
                    type="button"
                    class="btn text"
                    on:click=move |_| {
                        let month = c.calendar_month.get_untracked();
                        let today_d = today.get_untracked();
                        c.calendar_selected.update(|sel| {
                            let (from, to) = month_bounds(month);
                            let to = to.min(today_d);
                            let mut d = from;
                            while d <= to {
                                sel.insert(d);
                                d += Duration::days(1);
                            }
                        });
                    }
                >"Selecionar mês"</button>
                <button
                    type="button"
                    class="btn text"
                    on:click=move |_| c.calendar_selected.update(|s| s.clear())
                >"Limpar"</button>
            </div>
        </section>

        <PeriodResult usage=usage />
    }
}

#[component]
fn CalendarGrid(today: Memo<NaiveDate>) -> impl IntoView {
    let c = ctx();
    let cells = Memo::new(move |_| grid_cells(c.calendar_month.get()));
    let end_paint = move |_: PointerEvent| PAINTING.with(|p| p.set(None));

    view! {
        <div class="calendar" on:pointerup=end_paint on:pointerleave=end_paint on:pointercancel=end_paint>
            <div class="calendar-dow">
                {["seg", "ter", "qua", "qui", "sex", "sáb", "dom"]
                    .iter()
                    .map(|d| view! { <span class="muted small center">{*d}</span> })
                    .collect_view()}
            </div>
            <div class="calendar-grid">
                {move || cells.get().into_iter().map(|cell| day_cell(cell, today)).collect_view()}
            </div>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DayCell {
    date: NaiveDate,
    outside: bool,
}

fn day_cell(cell: DayCell, today: Memo<NaiveDate>) -> impl IntoView {
    let c = ctx();
    let date = cell.date;
    let future = move || date > today.get();
    let selected = move || c.calendar_selected.with(|s| s.contains(&date));
    let paint_to = move |target: bool| {
        if future() {
            return;
        }
        c.calendar_selected.update(|s| {
            if target {
                s.insert(date);
            } else {
                s.remove(&date);
            }
        });
    };
    let start = move |ev: PointerEvent| {
        if future() {
            return;
        }
        ev.prevent_default();
        let target = !selected();
        PAINTING.with(|p| p.set(Some(target)));
        paint_to(target);
    };
    let enter = move |_: PointerEvent| {
        if let Some(target) = PAINTING.with(Cell::get) {
            paint_to(target);
        }
    };

    view! {
        <button
            type="button"
            class="calendar-cell"
            class:outside=cell.outside
            class:selected=selected
            class:today=move || date == today.get()
            class:future=future
            disabled=future
            aria-pressed=move || selected().to_string()
            aria-label=format!("{}", short_date(date))
            on:pointerdown=start
            on:pointerenter=enter
        >
            {date.day().to_string()}
        </button>
    }
}

#[component]
fn PeriodResult(usage: Memo<Usage>) -> impl IntoView {
    let empty = move || usage.with(|u| u.available == 0);
    let used = move || usage.with(|u| u.consumed());
    let available = move || usage.with(|u| u.available);
    let used_ratio = move || {
        let a = available();
        if a == 0 { 0.0 } else { used() as f64 / a as f64 }
    };
    let used_pct = move || (used_ratio() * 100.0).round() as i32;

    view! {
        <section class="card">
            <Show
                when=move || !empty()
                fallback=|| view! {
                    <p class="muted center">
                        "Selecione dias com registros pra ver o gráfico do tempo livre."
                    </p>
                }
            >
                <div class="donut-row">
                    <Donut usage=usage used_pct=Signal::derive(used_pct) />
                    <div class="donut-legend stack">
                        <p class="muted small">
                            {move || format!(
                                "Tempo livre total {} (24h menos os blocos reservados).",
                                short(available())
                            )}
                        </p>
                        {move || usage.with(|u| {
                            let total = u.available as f64;
                            let used = u.consumed();
                            let mut rows: Vec<_> = u.tags.iter().map(|t| {
                                let pct = if total > 0.0 { t.consumed as f64 / total * 100.0 } else { 0.0 };
                                view! {
                                    <div class="donut-item row">
                                        <span class="dot" style:background=css(t.color)></span>
                                        <span class="grow ellipsis">{t.name.clone()}</span>
                                        <span class="pct sm">{format!("{}%", pct.round() as i32)}</span>
                                        <span class="muted small">{short(t.consumed)}</span>
                                    </div>
                                }.into_any()
                            }).collect();
                            let unused = u.available - used;
                            if unused > 0 {
                                let pct = unused as f64 / total * 100.0;
                                rows.push(view! {
                                    <div class="donut-item row">
                                        <span class="dot" style:background="var(--bg-3)"></span>
                                        <span class="grow ellipsis muted">"Não usado"</span>
                                        <span class="pct sm muted">{format!("{}%", pct.round() as i32)}</span>
                                        <span class="muted small">{short(unused)}</span>
                                    </div>
                                }.into_any());
                            }
                            rows
                        })}
                    </div>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn Donut(usage: Memo<Usage>, used_pct: Signal<i32>) -> impl IntoView {
    // Circunferência unitária (100): cada fatia = pct ocupa esse tanto no stroke-dasharray.
    let slices = Memo::new(move |_| {
        usage.with(|u| {
            if u.available == 0 {
                return Vec::new();
            }
            let total = u.available as f64;
            let mut offset = 0.0_f64;
            u.tags
                .iter()
                .filter(|t| t.consumed > 0)
                .map(|t| {
                    let pct = t.consumed as f64 / total * 100.0;
                    let dash = format!("{:.2} {:.2}", pct, 100.0 - pct);
                    let start = offset;
                    offset += pct;
                    (css(t.color), dash, -start)
                })
                .collect::<Vec<_>>()
        })
    });
    view! {
        <div class="donut">
            <svg viewBox="0 0 42 42" role="img" aria-label="Divisão do tempo livre">
                <circle class="donut-bg" cx="21" cy="21" r="15.915" fill="none" stroke-width="6"></circle>
                {move || slices.get().into_iter().map(|(color, dash, offset)| view! {
                    <circle
                        cx="21" cy="21" r="15.915" fill="none" stroke-width="6" stroke-linecap="butt"
                        stroke=color
                        stroke-dasharray=dash
                        stroke-dashoffset=format!("{:.2}", offset)
                    ></circle>
                }).collect_view()}
            </svg>
            <div class="donut-center">
                <strong class="pct lg">{move || format!("{}%", used_pct.get())}</strong>
                <span class="muted small center">"do tempo livre usado"</span>
            </div>
        </div>
    }
}

// -------- utilidades de data --------

fn month_bounds(month_start: NaiveDate) -> (NaiveDate, NaiveDate) {
    let next = shift_months(month_start, 1);
    (month_start, next - Duration::days(1))
}

fn shift_months(d: NaiveDate, delta: i32) -> NaiveDate {
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

fn month_title(d: NaiveDate) -> String {
    let m = match d.month() {
        1 => "janeiro",
        2 => "fevereiro",
        3 => "março",
        4 => "abril",
        5 => "maio",
        6 => "junho",
        7 => "julho",
        8 => "agosto",
        9 => "setembro",
        10 => "outubro",
        11 => "novembro",
        12 => "dezembro",
        _ => "",
    };
    format!("{m} de {}", d.year())
}

/// 42 células (6 semanas × 7 dias), com os dias de fora do mês marcados como `outside`.
fn grid_cells(month_start: NaiveDate) -> Vec<DayCell> {
    let first_dow = month_start.weekday().num_days_from_monday() as i64;
    let start = month_start - Duration::days(first_dow);
    (0..42)
        .map(|i| {
            let date = start + Duration::days(i);
            DayCell { date, outside: date.month() != month_start.month() || date.year() != month_start.year() }
        })
        .collect()
}

