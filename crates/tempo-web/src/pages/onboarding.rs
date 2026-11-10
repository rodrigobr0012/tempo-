//! Primeira execução (RF04, RF14): tempo reservado e início do dia → tags sugeridas → começar.

use leptos::prelude::*;
use tempo_core::reserved::DEFAULT_DAY_START;
use tempo_core::tags::SUGGESTED_TAGS;
use tempo_core::Permille;
use tempo_store::NewTag;

use crate::pages::plan::reserved_summary;
use crate::pages::ReservedList;
use crate::state::{ctx, Page};
use crate::ui::{clock_time, css, icon, Icon, TimeField};

#[component]
pub fn Onboarding() -> impl IntoView {
    let c = ctx();
    let step = RwSignal::new(0u8);
    let day_start = RwSignal::new(u16::from(DEFAULT_DAY_START.0) * 60 + u16::from(DEFAULT_DAY_START.1));
    let checked: Vec<RwSignal<bool>> = SUGGESTED_TAGS.iter().map(|_| RwSignal::new(true)).collect();
    let checked = StoredValue::new(checked);
    let reserved_total = Memo::new(move |_| c.read(|t| t.reserved_total()).unwrap_or(0));

    let finish = move |_| {
        let tags: Vec<NewTag> = SUGGESTED_TAGS
            .iter()
            .zip(checked.get_value())
            .filter(|(_, on)| on.get_untracked())
            .filter_map(|(&(name, p, color), _)| {
                Some(NewTag { name: name.into(), color, permille: Permille::new(p).ok()? })
            })
            .collect();
        let start = clock_time(i32::from(day_start.get_untracked()));
        if c.act(|t| t.complete_onboarding(start, &tags)).is_some() {
            c.request_notifications();
            c.page.set(Page::Today);
            c.tick();
        }
    };

    view! {
        <main class="onboarding">
            <p class="step">{move || format!("Passo {} de 2", step.get() + 1)}</p>
            <Show
                when=move || step.get() == 0
                fallback=move || view! {
                    <h1>"Como quer dividir?"</h1>
                    <p class="muted lead">
                        "Escolha algumas tags para começar. Você ajusta nomes e porcentagens depois, na aba Planejar."
                    </p>
                    <div class="stack">
                        {SUGGESTED_TAGS
                            .iter()
                            .zip(checked.get_value())
                            .map(|(&(name, p, color), on)| {
                                let pct = Permille::new(p).map(|p| p.to_string()).unwrap_or_default();
                                view! {
                                    <label class="tag-row check" style:border-color=move || on.get().then(|| css(color))>
                                        <input
                                            type="checkbox"
                                            prop:checked=move || on.get()
                                            on:change=move |ev| on.set(event_target_checked(&ev))
                                        />
                                        <span
                                            class="checkbox"
                                            style:background=move || on.get().then(|| css(color))
                                            style:border-color=move || on.get().then(|| css(color))
                                        >
                                            <Icon path=icon::CHECK />
                                        </span>
                                        <span class="tag-name grow">{name}</span>
                                        <span class="muted">{pct}</span>
                                    </label>
                                }
                            })
                            .collect_view()}
                    </div>
                    <p class="muted small">
                        "Para avisar quando o tempo de uma tag acabar, o navegador vai pedir permissão para enviar notificações."
                    </p>
                    <div class="row gap">
                        <button type="button" class="btn" on:click=move |_| step.set(0)>"Voltar"</button>
                        <button type="button" class="btn primary grow" on:click=finish>"Começar"</button>
                    </div>
                }
            >
                <h1>"Quanto tempo você reserva?"</h1>
                <p class="muted lead">
                    "Crie blocos para o tempo que não entra no planejamento: sono, faculdade, deslocamento… O que sobrar das 24h é o seu tempo disponível, dividido depois em porcentagens."
                </p>
                <ReservedList />
                <p class="summary">{move || reserved_summary(reserved_total.get())}</p>
                <section class="card">
                    <TimeField value=day_start label="Meu dia começa às" />
                    <p class="muted small">"Nesse horário os timers zeram e um novo dia começa."</p>
                </section>
                <button type="button" class="btn primary" on:click=move |_| step.set(1)>"Continuar"</button>
            </Show>
        </main>
    }
}
