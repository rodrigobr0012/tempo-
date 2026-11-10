//! Casca do app: carregamento do banco, navegação, avisos e atalhos de teclado.

use std::time::Duration;

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use tempo_core::format::hms;

use crate::browser;
use crate::dialogs::Dialogs;
use crate::pages::{HistoryPage, Onboarding, PlanPage, SettingsPage, TodayPage};
use crate::state::{self, ctx, Ctx, Page};
use crate::ui::{icon, Icon};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Boot {
    Loading,
    Ready,
    Failed,
}

#[component]
pub fn App() -> impl IntoView {
    let c = Ctx::new();
    provide_context(c);
    let boot = RwSignal::new(Boot::Loading);

    spawn_local(async move {
        boot.set(match start(c).await {
            Ok(()) => Boot::Ready,
            Err(e) => {
                leptos::logging::error!("{e}");
                Boot::Failed
            }
        });
    });

    view! {
        {move || match boot.get() {
            Boot::Loading => view! { <div class="splash">"Carregando…"</div> }.into_any(),
            Boot::Failed => view! {
                <div class="splash">
                    <p>"Não foi possível abrir os dados neste navegador."</p>
                    <p class="muted">"Verifique se o armazenamento do site não está bloqueado (janela anônima, por exemplo) e recarregue a página."</p>
                </div>
            }.into_any(),
            Boot::Ready => view! { <Shell /> }.into_any(),
        }}
    }
}

/// Carrega o banco salvo, prepara a primeira execução e liga o relógio de 1 s.
async fn start(c: Ctx) -> Result<(), String> {
    let saved = browser::load_db().await?;
    let bytes = saved.unwrap_or_default();
    if let Err(e) = state::load(&bytes) {
        // Cópia ilegível: guarda à parte e começa do zero, sem apagar o original.
        leptos::logging::error!("banco salvo não abriu: {e}");
        let _ = browser::backup_db(&bytes).await;
        state::load(&[]).map_err(|e| e.to_string())?;
        c.say("Não foi possível ler os dados salvos. Uma cópia foi guardada e o app começou do zero.");
    }
    browser::persist_storage();
    prepare_onboarding(c);
    c.tick();
    state::sync_desktop();

    browser::on_other_tab_saved(move || {
        spawn_local(async move {
            if let Ok(Some(bytes)) = browser::load_db().await {
                c.reload_from(&bytes);
            }
        });
    });
    set_interval(move || c.tick(), Duration::from_secs(1));
    Ok(())
}

/// Antes da primeira execução, sugere o bloco "Sono 8h" (RF04).
pub fn prepare_onboarding(c: Ctx) {
    let onboarded = state::with(|t| Ok(t.settings()?.onboarded)).unwrap_or(true);
    if !onboarded {
        c.act(|t| t.prepare_onboarding());
    }
}

#[component]
fn Shell() -> impl IntoView {
    let c = ctx();
    let onboarded = Memo::new(move |_| c.read(|t| Ok(t.settings()?.onboarded)).unwrap_or(false));

    // Título da aba com o timer em andamento.
    Effect::new(move |_| {
        c.now.track();
        let title = c
            .read(|t| t.active_timer())
            .flatten()
            .map(|a| {
                let p = a.progress(c.now.get_untracked());
                let time = if p.excess() > 0 { format!("+{}", hms(p.excess())) } else { hms(p.remaining()) };
                format!("{time} · {} — Tempo", a.tag_name)
            })
            .unwrap_or_else(|| "Tempo".into());
        document().set_title(&title);
    });

    // Esc fecha diálogos; espaço pausa/retoma o timer.
    let _keys = window_event_listener(ev::keydown, move |e| {
        if e.key() == "Escape" && c.dialog.get_untracked().is_some() {
            c.dialog.set(None);
            return;
        }
        let typing = document()
            .active_element()
            .map(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT" | "BUTTON"))
            .unwrap_or(false);
        if e.key() == " " && !typing && c.dialog.get_untracked().is_none() && onboarded.get_untracked() {
            e.prevent_default();
            crate::pages::toggle_hero(c);
        }
    });

    view! {
        <Show when=move || onboarded.get() fallback=|| view! { <Onboarding /> }>
            <div class="layout">
                <Nav />
                <main class="content">
                    {move || match c.page.get() {
                        Page::Today => view! { <TodayPage /> }.into_any(),
                        Page::Plan => view! { <PlanPage /> }.into_any(),
                        Page::History => view! { <HistoryPage /> }.into_any(),
                        Page::Settings => view! { <SettingsPage /> }.into_any(),
                    }}
                </main>
            </div>
        </Show>
        <Dialogs />
        <Toast />
    }
}

#[component]
fn Nav() -> impl IntoView {
    let c = ctx();
    let item = move |page: Page, path: &'static str, label: &'static str| {
        view! {
            <button
                type="button"
                class="nav-item"
                class:active=move || c.page.get() == page
                aria-current=move || (c.page.get() == page).then_some("page")
                on:click=move |_| {
                    c.dialog.set(None);
                    c.page.set(page);
                }
            >
                <span class="nav-pill"><Icon path=path /></span>
                <span class="nav-label">{label}</span>
            </button>
        }
    };
    view! {
        <nav class="nav" aria-label="Seções">
            <div class="brand">"Tempo"</div>
            {item(Page::Today, icon::TODAY, "Hoje")}
            {item(Page::Plan, icon::TUNE, "Planejar")}
            {item(Page::History, icon::CHART, "Histórico")}
            {item(Page::Settings, icon::SETTINGS, "Ajustes")}
        </nav>
    }
}

#[component]
fn Toast() -> impl IntoView {
    let c = ctx();
    view! {
        <div class="toast-area" aria-live="polite">
            {move || {
                c.toast.get().map(|(_, msg)| view! {
                    <button type="button" class="toast" on:click=move |_| c.toast.set(None)>{msg}</button>
                })
            }}
        </div>
    }
}
