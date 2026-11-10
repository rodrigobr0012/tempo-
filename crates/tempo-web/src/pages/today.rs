//! Painel do dia (RF25–RF27) com o timer (RF15–RF17).

use leptos::prelude::*;
use tempo_core::format::{hm, hms, short, tag_status};
use tempo_store::{TagToday, TodayView};

use crate::browser;
use crate::state::{ctx, Ctx, Dialog, Page};
use crate::ui::{css, icon, local_clock, Bar, Icon, IconButton};

/// Cartão principal: timer rodando ou tag pausada (foco).
#[derive(Clone, Debug, Default, PartialEq)]
struct HeroVm {
    tag_id: i64,
    name: String,
    color: String,
    running: bool,
    big: String,
    caption: String,
    progress: f32,
    excess: bool,
}

fn hero_vm(v: &TodayView) -> Option<HeroVm> {
    let (t, running) = match &v.active {
        Some(a) => (v.tags.iter().find(|t| t.tag.id == a.tag_id)?, true),
        None => (v.tags.iter().find(|t| Some(t.tag.id) == v.focus_tag)?, false),
    };
    let p = t.progress();
    let (big, caption) = if p.excess() > 0 {
        (format!("+{}", hms(p.excess())), format!("Excedente · meta de {} cumprida", short(t.allocated)))
    } else if running {
        let ends = v.now + p.remaining();
        (hms(p.remaining()), format!("restantes de {} · termina às {}", short(t.allocated), local_clock(ends)))
    } else {
        (hms(p.remaining()), format!("restantes de {}", short(t.allocated)))
    };
    Some(HeroVm {
        tag_id: t.tag.id,
        name: t.tag.name.clone(),
        color: css(t.tag.color),
        running,
        big,
        caption,
        progress: p.ratio(),
        excess: p.excess() > 0,
    })
}

/// Inicia a tag ou pausa, se ela já está rodando.
fn toggle_tag(c: Ctx, id: i64) {
    browser::unlock_audio();
    let running = crate::state::with(|t| Ok(t.active_timer()?.is_some_and(|a| a.tag_id == id))).unwrap_or(false);
    if running {
        c.act(|t| t.pause());
    } else {
        c.act(|t| t.start(id));
    }
}

/// Pausa o timer em andamento ou retoma a tag em foco (atalho: espaço).
pub fn toggle_hero(c: Ctx) {
    let target = crate::state::with(|t| {
        let v = t.today()?;
        Ok(v.active.as_ref().map(|a| a.tag_id).or(v.focus_tag))
    });
    if let Ok(Some(id)) = target {
        toggle_tag(c, id);
    }
}

#[component]
pub fn TodayPage() -> impl IntoView {
    let c = ctx();
    let today = Memo::new(move |_| {
        c.now.track();
        c.read(|t| t.today())
    });
    let ids = Memo::new(move |_| {
        today.with(|v| v.as_ref().map(|v| v.tags.iter().map(|t| t.tag.id).collect::<Vec<_>>()).unwrap_or_default())
    });
    let has_tags = Memo::new(move |_| ids.with(|i| !i.is_empty()));
    let hero = Memo::new(move |_| today.with(|v| v.as_ref().and_then(hero_vm)));
    let day = move |f: fn(&TodayView) -> String| move || today.with(|v| v.as_ref().map(f).unwrap_or_default());
    let flag = move |f: fn(&TodayView) -> bool| move || today.with(|v| v.as_ref().is_some_and(f));

    view! {
        <header class="page-head">
            <div class="grow">
                <h1>"Hoje"</h1>
                <p class="muted">{day(|v| tempo_core::format::day_title(v.date))}</p>
            </div>
            <IconButton
                path=icon::LIST
                label="Sessões de hoje".to_string()
                on_click=move || c.dialog.set(Some(Dialog::Sessions))
            />
            <IconButton
                path=icon::ADD
                label="Lançar tempo manualmente".to_string()
                disabled=Signal::derive(move || !has_tags.get())
                on_click=move || c.dialog.set(Some(Dialog::Manual))
            />
        </header>

        <div class="today-grid">
            <div class="stack">
                <section class="card">
                    <div class="row baseline">
                        <span class="big-number grow">{day(|v| hm(v.available))}</span>
                        <span class="muted">"disponíveis hoje"</span>
                    </div>
                    <StackedBar today=today />
                    <div class="row muted small">
                        <span class="grow">{day(|v| format!("Dia começou há {}", short(v.day_elapsed)))}</span>
                        <span>{day(|v| format!("Fim do dia em {}", short(v.day_remaining)))}</span>
                    </div>
                </section>

                <Show when=flag(|v| v.clock_warning)>
                    <div class="banner" role="alert">
                        <Icon path=icon::WARNING />
                        <span class="grow">
                            "O relógio do aparelho voltou no tempo. O timer foi encerrado no último horário válido."
                        </span>
                        <button type="button" class="btn text" on:click=move |_| {
                            c.act(|t| t.dismiss_clock_warning());
                        }>"OK"</button>
                    </div>
                </Show>

                <Show when=flag(|v| v.overcommitted)>
                    <div class="banner" role="status">
                        <Icon path=icon::WARNING />
                        <span class="grow">
                            {day(|v| format!(
                                "Faltam {} nas tags e restam {} até o fim do dia.",
                                short(v.total_remaining()),
                                short(v.day_remaining)
                            ))}
                        </span>
                    </div>
                </Show>

                <Show
                    when=move || hero.with(Option::is_some)
                    fallback=move || view! {
                        <Show when=move || has_tags.get()>
                            <p class="muted center hint">"Clique numa tag para iniciar o timer."</p>
                        </Show>
                    }
                >
                    <Hero hero=hero />
                </Show>
            </div>

            <div class="stack">
                <For each=move || ids.get() key=|id| *id let:id>
                    <TagRow id=id today=today />
                </For>

                <Show when=move || has_tags.get()>
                    <div class="tag-row unallocated">
                        <span class="stripe"></span>
                        <span class="grow">"Não alocado"</span>
                        <span class="muted small">
                            {day(|v| format!("{} · {}", v.unallocated_permille, short(v.unallocated_secs)))}
                        </span>
                    </div>
                </Show>

                <Show when=move || !has_tags.get() && today.with(Option::is_some)>
                    <section class="card">
                        <h2>"Nenhuma tag planejada"</h2>
                        <p class="muted">"Divida o seu tempo disponível em porcentagens: estudo, trabalho, lazer…"</p>
                        <button type="button" class="btn primary" on:click=move |_| c.page.set(Page::Plan)>
                            <Icon path=icon::TUNE />
                            "Planejar tags"
                        </button>
                    </section>
                </Show>
            </div>
        </div>
    }
}

#[component]
fn StackedBar(today: Memo<Option<TodayView>>) -> impl IntoView {
    let segments = move || {
        today.with(|v| {
            v.as_ref()
                .map(|v| v.tags.iter().map(|t| (css(t.tag.color), t.permille.fraction() * 100.0)).collect::<Vec<_>>())
                .unwrap_or_default()
        })
    };
    view! {
        <div class="stacked" role="img" aria-label="Divisão do tempo disponível entre as tags">
            {move || {
                segments()
                    .into_iter()
                    .map(|(color, pct)| {
                        view! { <span style:background=color style:width=format!("{pct:.2}%")></span> }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[component]
fn Hero(hero: Memo<Option<HeroVm>>) -> impl IntoView {
    let c = ctx();
    let s = move |f: fn(&HeroVm) -> String| move || hero.with(|h| h.as_ref().map(f).unwrap_or_default());
    let running = move || hero.with(|h| h.as_ref().is_some_and(|h| h.running));
    let excess = move || hero.with(|h| h.as_ref().is_some_and(|h| h.excess));
    let color = Signal::derive(move || hero.with(|h| h.as_ref().map(|h| h.color.clone()).unwrap_or_default()));
    let progress = Signal::derive(move || hero.with(|h| h.as_ref().map_or(0.0, |h| h.progress)));

    view! {
        <section class="card hero" class:running=running style:border-color=move || running().then(|| color.get())>
            <div class="row">
                <span class="dot" style:background=move || color.get()></span>
                <h2 class="grow ellipsis">{s(|h| h.name.clone())}</h2>
                <span class="status" class:ok=running>{move || if running() { "Rodando" } else { "Pausado" }}</span>
            </div>
            <div class="hero-time" class:danger=excess role="timer" aria-live="off">{s(|h| h.big.clone())}</div>
            <p class="muted center">{s(|h| h.caption.clone())}</p>
            <Bar
                progress=progress
                color=Signal::derive(move || if excess() { "var(--danger)".into() } else { color.get() })
            />
            <div class="row center gap">
                <button
                    type="button"
                    class="btn primary"
                    title="Atalho: espaço"
                    on:click=move |_| {
                        if let Some(id) = hero.with_untracked(|h| h.as_ref().map(|h| h.tag_id)) {
                            toggle_tag(c, id);
                        }
                    }
                >
                    {move || view! { <Icon path=if running() { icon::PAUSE } else { icon::PLAY } /> }}
                    {move || if running() { "Pausar" } else { "Retomar" }}
                </button>
                <button type="button" class="btn" on:click=move |_| {
                    c.act(|t| t.stop());
                }>
                    <Icon path=icon::STOP />
                    "Parar"
                </button>
            </div>
        </section>
    }
}

#[component]
fn TagRow(id: i64, today: Memo<Option<TodayView>>) -> impl IntoView {
    let c = ctx();
    let row =
        Memo::new(move |_| today.with(|v| v.as_ref().and_then(|v| v.tags.iter().find(|t| t.tag.id == id).cloned())));
    let s = move |f: fn(&TagToday) -> String| move || row.with(|r| r.as_ref().map(f).unwrap_or_default());
    let b = move |f: fn(&TagToday) -> bool| move || row.with(|r| r.as_ref().is_some_and(f));
    let running = b(|t| t.running);
    let excess = b(|t| t.progress().excess() > 0);
    let finished = b(|t| t.progress().is_finished() && t.allocated > 0);
    let color = Signal::derive(s(|t| css(t.tag.color)));
    let progress = Signal::derive(move || row.with(|r| r.as_ref().map_or(0.0, |t| t.progress().ratio())));

    view! {
        <button
            type="button"
            class="tag-row"
            class:running=running
            style:border-color=move || running().then(|| color.get())
            aria-pressed=move || running().to_string()
            on:click=move |_| toggle_tag(c, id)
        >
            <span class="stripe" style:background=move || color.get()></span>
            <span class="tag-body">
                <span class="row">
                    <span class="tag-name grow ellipsis">{s(|t| t.tag.name.clone())}</span>
                    <span class="muted small">{s(|t| t.permille.to_string())}</span>
                </span>
                <Bar
                    progress=progress
                    color=Signal::derive(move || if excess() { "var(--danger)".into() } else { color.get() })
                />
                <span class="row small">
                    <span class="muted grow ellipsis">
                        {s(|t| format!("{} de {}", short(t.consumed), short(t.allocated)))}
                    </span>
                    <span class="strong" class:danger=excess class:ok=move || finished() && !excess()>
                        {s(|t| tag_status(t.progress()))}
                    </span>
                </span>
            </span>
            <span class="play" style:background=move || running().then(|| color.get())>
                {move || view! { <Icon path=if running() { icon::PAUSE } else { icon::PLAY } /> }}
            </span>
        </button>
    }
}
