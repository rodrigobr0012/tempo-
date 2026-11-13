//! Peças comuns da interface: ícones, cores, barras e campos de hora/duração.

use chrono::Timelike;
use leptos::prelude::*;
use tempo_core::{Clock, ClockTime, SystemClock};

/// Ícones Material (viewbox 24×24), os mesmos do app Android.
pub mod icon {
    pub const PLAY: &str = "M8 5v14l11-7z";
    pub const PAUSE: &str = "M6 19h4V5H6v14zm8-14v14h4V5h-4z";
    pub const STOP: &str = "M6 6h12v12H6z";
    pub const ADD: &str = "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";
    pub const TODAY: &str = "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z";
    pub const TUNE: &str = "M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z";
    pub const CHART: &str = "M5 9.2h3V19H5zM10.6 5h2.8v14h-2.8zm5.6 8H19v6h-2.8z";
    pub const SETTINGS: &str = "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z";
    pub const EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";
    pub const DELETE: &str = "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";
    pub const UP: &str = "M7.41 15.41L12 10.83l4.59 4.58L18 14l-6-6-6 6z";
    pub const DOWN: &str = "M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z";
    pub const CLOSE: &str =
        "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";
    pub const LIST: &str = "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z";
    pub const ARCHIVE: &str = "M20.54 5.23l-1.39-1.68C18.88 3.21 18.47 3 18 3H6c-.47 0-.88.21-1.16.55L3.46 5.23C3.17 5.57 3 6.02 3 6.5V19c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V6.5c0-.48-.17-.93-.46-1.27zM12 17.5L6.5 12H10v-2h4v2h3.5L12 17.5zM5.12 5l.81-1h12l.94 1H5.12z";
    pub const UNARCHIVE: &str = "M20.55 5.22l-1.39-1.68C18.88 3.21 18.47 3 18 3H6c-.47 0-.88.21-1.15.55L3.46 5.22C3.17 5.57 3 6.01 3 6.5V19c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6.5c0-.49-.17-.93-.45-1.28zM12 9.5l5.5 5.5H14v2h-4v-2H6.5L12 9.5zM5.12 5l.82-1h12l.93 1H5.12z";
    pub const WARNING: &str = "M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z";
    pub const CHECK: &str = "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z";
    pub const GLASS: &str = "M3 2l2.01 18.23C5.13 21.23 5.97 22 7 22h10c1.03 0 1.87-.77 1.99-1.77L21 2H3zm9 17c-1.66 0-3-1.34-3-3 0-2 3-5.4 3-5.4s3 3.4 3 5.4c0 1.66-1.34 3-3 3zm6.33-11H5.67l-.44-4h13.53l-.43 4z";
    pub const TIMER: &str = "M15 1H9v2h6V1zm-4 13h2V8h-2v6zm8.03-6.61l1.42-1.42c-.43-.51-.9-.99-1.41-1.41l-1.42 1.42C16.07 4.74 14.12 4 12 4c-4.97 0-9 4.03-9 9s4.02 9 9 9 9-4.03 9-9c0-2.12-.74-4.07-1.97-5.61zM12 20c-3.87 0-7-3.13-7-7s3.13-7 7-7 7 3.13 7 7-3.13 7-7 7z";
}

#[component]
pub fn Icon(path: &'static str) -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d=path />
        </svg>
    }
}

/// Botão só com ícone; `label` vira o nome acessível e a dica.
#[component]
pub fn IconButton(
    path: &'static str,
    #[prop(into)] label: Signal<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(optional)] danger: bool,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <button
            class="icon-btn"
            class:danger=danger
            type="button"
            aria-label=move || label.get()
            title=move || label.get()
            disabled=move || disabled.get()
            on:click=move |_| on_click()
        >
            <Icon path=path />
        </button>
    }
}

/// `#RRGGBB` para o CSS.
pub fn css(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
}

/// Barra de progresso (0–1).
#[component]
pub fn Bar(#[prop(into)] progress: Signal<f32>, #[prop(into)] color: Signal<String>) -> impl IntoView {
    view! {
        <div class="bar">
            <div
                class="bar-fill"
                style:width=move || format!("{:.2}%", progress.get().clamp(0.0, 1.0) * 100.0)
                style:background=move || color.get()
            ></div>
        </div>
    }
}

/// "HH:MM" local de um instante.
pub fn local_clock(ts: i64) -> String {
    SystemClock.utc_to_local(ts).format("%H:%M").to_string()
}

/// Minutos desde 00:00 no horário local.
pub fn local_minutes(ts: i64) -> u16 {
    let t = SystemClock.utc_to_local(ts);
    (t.hour() * 60 + t.minute()) as u16
}

pub fn clock_time(minutes: i32) -> ClockTime {
    ClockTime::from_minutes(minutes.rem_euclid(1440) as u16).expect("minutos válidos")
}

/// Campo de horário (`<input type="time">`) ligado a minutos desde 00:00.
#[component]
pub fn TimeField(value: RwSignal<u16>, #[prop(into)] label: String) -> impl IntoView {
    view! {
        <label class="field">
            <span class="field-label">{label}</span>
            <input
                type="time"
                step="300"
                required
                prop:value=move || ClockTime::from_minutes(value.get()).map(|t| t.to_string()).unwrap_or_default()
                on:change=move |ev| {
                    if let Ok(t) = event_target_value(&ev).parse::<ClockTime>() {
                        value.set(t.minutes());
                    }
                }
            />
        </label>
    }
}

/// Duração em horas + minutos, ligada a minutos totais.
#[component]
pub fn DurationField(
    minutes: RwSignal<i64>,
    #[prop(into)] label: String,
    #[prop(default = 23)] max_hours: i64,
) -> impl IntoView {
    let set = move |h: i64, m: i64| minutes.set((h.clamp(0, max_hours) * 60 + m.clamp(0, 59)).max(0));
    view! {
        <fieldset class="field duration">
            <legend class="field-label">{label}</legend>
            <div class="duration-inputs">
                <label>
                    <input
                        type="number"
                        min="0"
                        max=max_hours
                        inputmode="numeric"
                        prop:value=move || (minutes.get() / 60).to_string()
                        on:input=move |ev| {
                            let h = event_target_value(&ev).parse().unwrap_or(0);
                            set(h, minutes.get_untracked() % 60);
                        }
                    />
                    <span>"h"</span>
                </label>
                <label>
                    <input
                        type="number"
                        min="0"
                        max="59"
                        step="5"
                        inputmode="numeric"
                        prop:value=move || (minutes.get() % 60).to_string()
                        on:input=move |ev| {
                            let m = event_target_value(&ev).parse().unwrap_or(0);
                            set(minutes.get_untracked() / 60, m);
                        }
                    />
                    <span>"min"</span>
                </label>
            </div>
        </fieldset>
    }
}
