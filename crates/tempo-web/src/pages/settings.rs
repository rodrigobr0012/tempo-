//! Ajustes: início do dia, timer, notificações e dados (RF33–RF36).

use leptos::prelude::*;

use crate::browser::{self, Permission};
use crate::state::{ctx, Confirm};
use crate::ui::{clock_time, icon, Icon, TimeField};

const RETENTION_OPTIONS: [(Option<u32>, &str); 4] =
    [(None, "Para sempre"), (Some(90), "90 dias"), (Some(180), "180 dias"), (Some(365), "365 dias")];

#[component]
pub fn SettingsPage() -> impl IntoView {
    let c = ctx();
    let settings = Memo::new(move |_| c.read(|t| t.settings()));
    let day_start = RwSignal::new(settings.get_untracked().map_or(240, |s| s.day_start.minutes()));
    let can_vibrate = browser::can_vibrate();
    let desktop = browser::is_desktop();

    let toggle = move |label: &'static str,
                       detail: Option<&'static str>,
                       get: fn(&tempo_store::Settings) -> bool,
                       set: fn(&mut tempo_store::Tempo, bool) -> tempo_store::Result<()>| {
        let checked = move || settings.with(|s| s.as_ref().is_some_and(get));
        view! {
            <label class="toggle-row">
                <span class="grow">
                    <span class="toggle-title">{label}</span>
                    {detail.map(|d| view! { <span class="muted small block">{d}</span> })}
                </span>
                <input
                    type="checkbox"
                    role="switch"
                    class="switch"
                    prop:checked=checked
                    on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        c.act(|t| set(t, on));
                    }
                />
            </label>
        }
    };

    view! {
        <header class="page-head">
            <h1 class="grow">"Ajustes"</h1>
        </header>

        <div class="settings-grid">
            <div class="stack">
                <h2 class="section">"Dia"</h2>
                <section class="card">
                    <TimeField value=day_start label="Meu dia começa às" />
                    <p class="muted small">
                        "Nesse horário os timers zeram e começa um novo dia no histórico. Quem dorme tarde pode usar a madrugada (ex.: 04:00)."
                    </p>
                    <button type="button" class="btn primary" on:click=move |_| {
                        let m = i32::from(day_start.get_untracked());
                        if c.act(|t| t.set_day_start(clock_time(m))).is_some() {
                            c.say("Horário salvo. Vale a partir do próximo dia.");
                        }
                    }>"Salvar horário"</button>
                    <p class="muted small center">"O tempo reservado (sono e outros blocos) fica na aba Planejar."</p>
                </section>

                <h2 class="section">"Timer"</h2>
                <section class="card flush">
                    {toggle(
                        "Continuar no excedente",
                        Some("Depois de zerar, o timer segue contando o tempo extra."),
                        |s| s.overtime,
                        |t, v| t.set_overtime(v),
                    )}
                    {toggle("Som ao terminar", None, |s| s.sound, |t, v| t.set_sound(v))}
                    {can_vibrate.then(|| toggle("Vibrar ao terminar", None, |s| s.vibration, |t, v| t.set_vibration(v)))}
                </section>
            </div>

            <div class="stack">
                {if desktop {
                    view! { <DesktopSystem /> }.into_any()
                } else {
                    view! { <WebNotifications /> }.into_any()
                }}

                <h2 class="section">"Dados"</h2>
                <section class="card">
                    <label class="field">
                        <span class="field-label">"Manter histórico"</span>
                        <select on:change=move |ev| {
                            let i: usize = event_target_value(&ev).parse().unwrap_or(0);
                            let days = RETENTION_OPTIONS.get(i).and_then(|(d, _)| *d);
                            c.act(|t| t.set_retention(days));
                        }>
                            {RETENTION_OPTIONS
                                .iter()
                                .enumerate()
                                .map(|(i, (days, label))| {
                                    let days = *days;
                                    view! {
                                        <option
                                            value=i.to_string()
                                            selected=move || settings.with(|s| s.as_ref().is_some_and(|s| s.retention_days == days))
                                        >
                                            {*label}
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <button type="button" class="btn" on:click=move |_| {
                        if let Some(csv) = c.act(|t| t.export_csv()) {
                            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                            browser::download(&format!("tempo-sessoes-{stamp}.csv"), &csv, "text/csv;charset=utf-8");
                        }
                    }>"Exportar sessões (CSV)"</button>
                    <button type="button" class="btn danger" on:click=move |_| c.confirm(
                        "Apagar todos os dados?",
                        "Tags, sessões, histórico e ajustes serão apagados deste navegador. Isso não pode ser desfeito.",
                        "Apagar tudo",
                        Confirm::Erase,
                    )>
                        <Icon path=icon::DELETE />
                        "Apagar todos os dados"
                    </button>
                </section>
            </div>
        </div>

        <p class="muted small center footer">
            {format!(
                "Tempo {} · seus dados ficam só {}.",
                env!("CARGO_PKG_VERSION"),
                if desktop { "neste computador" } else { "neste navegador" }
            )}
        </p>
    }
}

/// Navegador: permissão de notificações.
#[component]
fn WebNotifications() -> impl IntoView {
    let c = ctx();
    view! {
        <h2 class="section">"Notificações"</h2>
        <section class="card">
            <p class="strong">
                {move || match c.notif.get() {
                    Permission::Granted => "Notificações permitidas",
                    Permission::Denied => "Notificações bloqueadas",
                    Permission::Default => "Notificações ainda não permitidas",
                    Permission::Unsupported => "Este navegador não oferece notificações",
                }}
            </p>
            <p class="muted small">
                "Usadas para avisar quando o tempo de uma tag acabar. Com a aba em segundo plano, o navegador pode atrasar o aviso em até um minuto; com o navegador fechado, não há aviso."
            </p>
            <Show when=move || c.notif.get() == Permission::Default>
                <button type="button" class="btn" on:click=move |_| c.request_notifications()>
                    "Permitir notificações"
                </button>
            </Show>
            <Show when=move || c.notif.get() == Permission::Denied>
                <p class="muted small">"Para liberar, use o ícone ao lado do endereço do site."</p>
            </Show>
        </section>
    }
}

/// Programa para Windows: início automático e bandeja.
#[component]
fn DesktopSystem() -> impl IntoView {
    let c = ctx();
    let autostart = RwSignal::new(false);
    leptos::task::spawn_local(async move { autostart.set(browser::autostart_enabled().await) });
    view! {
        <h2 class="section">"Windows"</h2>
        <section class="card flush">
            <label class="toggle-row">
                <span class="grow">
                    <span class="toggle-title">"Iniciar com o Windows"</span>
                    <span class="muted small block">"Abre sozinho, direto no ícone perto do relógio."</span>
                </span>
                <input
                    type="checkbox"
                    role="switch"
                    class="switch"
                    prop:checked=move || autostart.get()
                    on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        leptos::task::spawn_local(async move {
                            match browser::set_autostart(on).await {
                                Ok(()) => autostart.set(on),
                                Err(e) => {
                                    autostart.set(!on);
                                    c.say(format!("Não foi possível mudar o início automático: {e}"));
                                }
                            }
                        });
                    }
                />
            </label>
            <p class="muted small tray-note">
                "Fechar a janela não encerra o Tempo: ele continua no ícone perto do relógio e avisa com uma notificação do Windows quando o tempo de uma tag acabar. Para sair de vez, clique no ícone com o botão direito."
            </p>
        </section>
    }
}
