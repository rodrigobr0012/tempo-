//! Histórico por dia e estatísticas (RF30, RF31).

use leptos::prelude::*;
use tempo_core::format::{short, short_date};
use tempo_core::Progress;
use tempo_store::{DaySummary, StatsView};

use crate::state::{ctx, HistoryTab};
use crate::ui::{css, Bar};

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
            HistoryTab::Days => return StatsView::default(),
            HistoryTab::Week => 7,
            HistoryTab::Month => 30,
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
                                            <span class="strong">{format!("{}%", (st.compliance * 100.0).round() as i32)}</span>
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
