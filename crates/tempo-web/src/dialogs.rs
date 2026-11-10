//! Diálogos: editor de tag (RF06–RF08), lançamento manual (RF23), sessões de hoje (RF24),
//! bloco reservado (RF01–RF03) e confirmação.

use leptos::prelude::*;
use tempo_core::format::short;
use tempo_core::reserved::check_reserved;
use tempo_core::tags::PALETTE;
use tempo_core::{allocated_secs, available_secs, Permille};
use tempo_store::{NewReserved, NewTag, Origin};

use crate::app::prepare_onboarding;
use crate::state::{self, ctx, Confirm, Dialog, Page};
use crate::ui::{clock_time, css, icon, local_clock, local_minutes, DurationField, Icon, IconButton, TimeField};

#[component]
pub fn Dialogs() -> impl IntoView {
    let c = ctx();
    move || {
        c.dialog.get().map(|d| match d {
            Dialog::TagEditor { id } => view! { <TagEditor id=id /> }.into_any(),
            Dialog::Manual => view! { <ManualDialog /> }.into_any(),
            Dialog::Sessions => view! { <SessionsDialog /> }.into_any(),
            Dialog::Reserved { id } => view! { <ReservedDialog id=id /> }.into_any(),
            Dialog::Confirm { title, text, button, action } => {
                view! { <ConfirmDialog title=title text=text button=button action=action /> }.into_any()
            }
        })
    }
}

/// Painel modal com título e botão de fechar; clicar fora fecha.
#[component]
fn Sheet(#[prop(into)] title: String, children: Children) -> impl IntoView {
    let c = ctx();
    let label = title.clone();
    view! {
        <div class="scrim" on:click=move |_| c.dialog.set(None)>
            <div
                class="sheet"
                role="dialog"
                aria-modal="true"
                aria-label=label
                on:click=|ev| ev.stop_propagation()
            >
                <div class="row">
                    <h2 class="grow">{title}</h2>
                    <IconButton path=icon::CLOSE label="Fechar".to_string() on_click=move || c.dialog.set(None) />
                </div>
                {children()}
            </div>
        </div>
    }
}

#[component]
fn TagEditor(id: Option<i64>) -> impl IntoView {
    let c = ctx();
    let (tags, available) = state::with(|t| Ok((t.tags(false)?, t.current_available()?))).unwrap_or_default();
    let current = id.and_then(|id| tags.iter().find(|t| t.id == id).cloned());
    let others: u32 = tags.iter().filter(|t| Some(t.id) != id).map(|t| u32::from(t.permille.value())).sum();
    let max = 1000 - others.min(1000) as u16;

    let name = RwSignal::new(current.as_ref().map(|t| t.name.clone()).unwrap_or_default());
    let color = RwSignal::new(current.as_ref().map_or(PALETTE[0], |t| t.color));
    let permille = RwSignal::new(current.as_ref().map_or(max.min(100), |t| t.permille.value()));
    let by_duration = RwSignal::new(false);
    let minutes = RwSignal::new(60i64);

    let effective = Memo::new(move |_| {
        if !by_duration.get() {
            return permille.get();
        }
        let m = minutes.get();
        if m <= 0 {
            return 0;
        }
        Permille::from_duration(m * 60, available).map_or(1001, Permille::value)
    });
    let valid = move || !name.with(|n| n.trim().is_empty()) && (1..=max).contains(&effective.get());
    let text = |p: u16| Permille::saturating(i64::from(p)).to_string();
    let dur = move |p: u16| short(allocated_secs(available, Permille::saturating(i64::from(p))));

    let save = move |_| {
        let Ok(p) = Permille::new(effective.get_untracked()) else { return };
        let new = NewTag { name: name.get_untracked(), color: color.get_untracked(), permille: p };
        let ok = match id {
            None => c.act(|t| t.create_tag(&new)).is_some(),
            Some(id) => c.act(|t| t.update_tag(id, &new)).is_some(),
        };
        if ok {
            c.dialog.set(None);
        }
    };

    let title = if id.is_some() { "Editar tag" } else { "Nova tag" };
    view! {
        <Sheet title=title>
            <form class="stack" on:submit=move |ev| {
                ev.prevent_default();
                if valid() {
                    save(());
                }
            }>
                <label class="field">
                    <span class="field-label">"Nome"</span>
                    <input
                        type="text"
                        maxlength="30"
                        placeholder="Ex.: Estudo"
                        autofocus
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </label>

                <fieldset class="field">
                    <legend class="field-label">"Cor"</legend>
                    <div class="swatches">
                        {PALETTE
                            .iter()
                            .map(|&p| view! {
                                <button
                                    type="button"
                                    class="swatch"
                                    style:background=css(p)
                                    aria-label=css(p)
                                    aria-pressed=move || (color.get() == p).to_string()
                                    on:click=move |_| color.set(p)
                                >
                                    <Show when=move || color.get() == p><Icon path=icon::CHECK /></Show>
                                </button>
                            })
                            .collect_view()}
                    </div>
                </fieldset>

                <div class="segmented" role="tablist">
                    <button
                        type="button"
                        role="tab"
                        class="seg"
                        class:active=move || !by_duration.get()
                        aria-selected=move || (!by_duration.get()).to_string()
                        on:click=move |_| by_duration.set(false)
                    >"Porcentagem"</button>
                    <button
                        type="button"
                        role="tab"
                        class="seg"
                        class:active=move || by_duration.get()
                        aria-selected=move || by_duration.get().to_string()
                        on:click=move |_| {
                            if !by_duration.get_untracked() {
                                // Começa do mesmo ponto da porcentagem atual.
                                let p = Permille::saturating(i64::from(permille.get_untracked()));
                                minutes.set((allocated_secs(available, p) / 60).max(5));
                            }
                            by_duration.set(true);
                        }
                    >"Duração"</button>
                </div>

                <Show
                    when=move || by_duration.get()
                    fallback=move || view! {
                        <p class="big-number center">
                            {move || format!("{} = {}", text(permille.get()), dur(permille.get()))}
                        </p>
                        <input
                            type="range"
                            min="1"
                            max=max.max(1)
                            step="5"
                            aria-label="Porcentagem"
                            prop:value=move || permille.get().to_string()
                            on:input=move |ev| {
                                let v: u16 = event_target_value(&ev).parse().unwrap_or(1);
                                permille.set(v.clamp(1, max.max(1)));
                            }
                        />
                        <label class="field inline">
                            <span class="field-label">"Porcentagem exata"</span>
                            <input
                                type="number"
                                min="0.1"
                                max=format!("{:.1}", f32::from(max.max(1)) / 10.0)
                                step="0.1"
                                inputmode="decimal"
                                prop:value=move || format!("{:.1}", f32::from(permille.get()) / 10.0)
                                on:change=move |ev| {
                                    let raw = event_target_value(&ev).replace(',', ".");
                                    if let Ok(v) = raw.parse::<f32>() {
                                        let p = (v * 10.0).round().clamp(1.0, f32::from(max.max(1))) as u16;
                                        permille.set(p);
                                    }
                                }
                            />
                            <span>"%"</span>
                        </label>
                    }
                >
                    <DurationField minutes=minutes label="Duração por dia" />
                    <p class="strong center">
                        {move || format!("= {} do tempo disponível", text(effective.get()))}
                    </p>
                </Show>

                <p class="muted center small" class:danger={move || effective.get() > max}>
                    {move || if max >= 1 {
                        format!("Disponível para esta tag: até {} ({}).", text(max), dur(max))
                    } else {
                        "Não há porcentagem livre. Reduza outra tag primeiro.".to_string()
                    }}
                </p>

                <button type="submit" class="btn primary" disabled=move || !valid()>"Salvar"</button>
            </form>

            {id.map(|id| view! {
                <div class="row gap">
                    <button type="button" class="btn grow" on:click=move |_| {
                        if c.act(|t| t.archive_tag(id)).is_some() {
                            c.dialog.set(None);
                            c.say("Tag arquivada. O histórico foi mantido.");
                        }
                    }>
                        <Icon path=icon::ARCHIVE />
                        "Arquivar"
                    </button>
                    <button type="button" class="btn danger grow" on:click=move |_| c.confirm(
                        format!("Excluir \"{}\"?", name.get_untracked()),
                        "A tag e todo o histórico dela serão apagados. Para manter o histórico, use Arquivar.",
                        "Excluir",
                        Confirm::DeleteTag(id),
                    )>
                        <Icon path=icon::DELETE />
                        "Excluir"
                    </button>
                </div>
            })}
        </Sheet>
    }
}

#[component]
fn ManualDialog() -> impl IntoView {
    let c = ctx();
    let tags: Vec<(i64, String)> = state::with(|t| t.today())
        .map(|v| v.tags.into_iter().map(|t| (t.tag.id, t.tag.name)).collect())
        .unwrap_or_default();
    let tag = RwSignal::new(tags.first().map_or(-1, |t| t.0));
    let now_min = i32::from(local_minutes(chrono::Utc::now().timestamp()));
    let start = RwSignal::new(((now_min - 30).rem_euclid(1440) / 5 * 5) as u16);
    let minutes = RwSignal::new(30i64);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let tag_id = tag.get_untracked();
        if tag_id < 0 {
            c.say("Escolha uma tag.");
            return;
        }
        let secs = minutes.get_untracked() * 60;
        let at = clock_time(i32::from(start.get_untracked()));
        let ok = c
            .act(|t| {
                let start = t.day_time_to_utc(at)?;
                t.add_manual(tag_id, start, secs)
            })
            .is_some();
        if ok {
            c.dialog.set(None);
            c.say("Tempo lançado.");
        }
    };

    view! {
        <Sheet title="Lançar tempo">
            <form class="stack" on:submit=submit>
                <p class="muted">"Esqueceu de ligar o timer? Registre o tempo aqui."</p>
                <label class="field">
                    <span class="field-label">"Tag"</span>
                    <select on:change=move |ev| tag.set(event_target_value(&ev).parse().unwrap_or(-1))>
                        {tags
                            .into_iter()
                            .map(|(id, name)| view! {
                                <option value=id.to_string() selected=move || tag.get() == id>{name}</option>
                            })
                            .collect_view()}
                    </select>
                </label>
                <TimeField value=start label="Início" />
                <DurationField minutes=minutes label="Duração" />
                <button type="submit" class="btn primary" disabled=move || minutes.get() <= 0>"Lançar"</button>
            </form>
        </Sheet>
    }
}

#[component]
fn SessionsDialog() -> impl IntoView {
    let c = ctx();
    let sessions = Memo::new(move |_| {
        c.now.track();
        c.read(|t| t.sessions_today()).unwrap_or_default()
    });
    view! {
        <Sheet title="Sessões de hoje">
            <Show when=move || sessions.with(Vec::is_empty)>
                <p class="muted">"Nenhuma sessão registrada hoje."</p>
            </Show>
            <ul class="sessions">
                {move || {
                    let now = c.now.get();
                    sessions.with(|list| {
                        list.iter()
                            .map(|s| {
                                let id = s.id;
                                let range = match s.end {
                                    Some(e) => format!("{}–{}", local_clock(s.start), local_clock(e)),
                                    None => format!("{}–agora", local_clock(s.start)),
                                };
                                let duration = short((s.end.unwrap_or(now) - s.start).max(0));
                                let manual = if s.origin == Origin::Manual { " · manual" } else { "" };
                                let label = format!("Apagar sessão de {}, {}", s.tag_name, range);
                                view! {
                                    <li class="row">
                                        <span class="dot" style:background=css(s.color)></span>
                                        <span class="grow">
                                            <span class="block ellipsis">{format!("{}{manual}", s.tag_name)}</span>
                                            <span class="muted small">{format!("{range} · {duration}")}</span>
                                        </span>
                                        {if s.end.is_some() {
                                            view! {
                                                <IconButton
                                                    path=icon::DELETE
                                                    label=label
                                                    danger=true
                                                    on_click=move || {
                                                        if c.act(|t| t.delete_session(id)).is_some() {
                                                            c.say("Sessão apagada.");
                                                        }
                                                    }
                                                />
                                            }
                                            .into_any()
                                        } else {
                                            view! { <span class="strong ok small">"agora"</span> }.into_any()
                                        }}
                                    </li>
                                }
                            })
                            .collect_view()
                    })
                }}
            </ul>
        </Sheet>
    }
}

#[component]
fn ReservedDialog(id: Option<i64>) -> impl IntoView {
    let c = ctx();
    let list = state::with(|t| t.reserved()).unwrap_or_default();
    let current = id.and_then(|id| list.iter().find(|r| r.id == id).cloned());
    let others: i64 = list.iter().filter(|r| Some(r.id) != id).map(|r| r.duration).sum();
    let onboarded = state::with(|t| Ok(t.settings()?.onboarded)).unwrap_or(false);

    let name = RwSignal::new(current.as_ref().map(|r| r.name.clone()).unwrap_or_default());
    let minutes = RwSignal::new(current.as_ref().map_or(60, |r| r.duration / 60));
    let check = move || check_reserved(others, minutes.get() * 60);
    let valid = move || !name.with(|n| n.trim().is_empty()) && minutes.get() >= 15 && check().is_ok();

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if !valid() {
            return;
        }
        let new = NewReserved { name: name.get_untracked(), duration: minutes.get_untracked() * 60 };
        let ok = match id {
            None => c.act(|t| t.create_reserved(&new)).is_some(),
            Some(id) => c.act(|t| t.update_reserved(id, &new)).is_some(),
        };
        if ok {
            c.dialog.set(None);
            if onboarded {
                c.say("Salvo. O tempo disponível muda a partir do próximo dia.");
            }
        }
    };

    let title = if id.is_some() { "Editar bloco reservado" } else { "Novo bloco reservado" };
    view! {
        <Sheet title=title>
            <form class="stack" on:submit=submit>
                <label class="field">
                    <span class="field-label">"Nome"</span>
                    <input
                        type="text"
                        maxlength="30"
                        placeholder="Ex.: Sono, Faculdade"
                        autofocus
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </label>
                <DurationField minutes=minutes label="Duração por dia" />
                <p class="strong center" class:danger=move || check().is_err() || minutes.get() < 15>
                    {move || {
                        if minutes.get() < 15 {
                            return "O bloco precisa ter pelo menos 15min.".to_string();
                        }
                        match check() {
                            Ok(()) => format!(
                                "Sobram {} disponíveis por dia",
                                short(available_secs(others + minutes.get() * 60))
                            ),
                            Err(e) => e.to_string(),
                        }
                    }}
                </p>
                <button type="submit" class="btn primary" disabled=move || !valid()>"Salvar"</button>
            </form>
            {id.map(|id| view! {
                <button type="button" class="btn danger" on:click=move |_| c.confirm(
                    format!("Excluir \"{}\"?", name.get_untracked()),
                    "O tempo deste bloco volta a ficar disponível a partir do próximo dia.",
                    "Excluir",
                    Confirm::DeleteReserved(id),
                )>
                    <Icon path=icon::DELETE />
                    "Excluir"
                </button>
            })}
        </Sheet>
    }
}

#[component]
fn ConfirmDialog(title: String, text: String, button: String, action: Confirm) -> impl IntoView {
    let c = ctx();
    let accept = move |_| {
        c.dialog.set(None);
        match action.clone() {
            Confirm::Erase => {
                if c.act(|t| t.erase_all()).is_some() {
                    c.page.set(Page::Today);
                    prepare_onboarding(c);
                }
            }
            Confirm::DeleteTag(id) => {
                if c.act(|t| t.delete_tag(id)).is_some() {
                    c.say("Tag excluída.");
                }
            }
            Confirm::DeleteReserved(id) => {
                if c.act(|t| t.delete_reserved(id)).is_some() {
                    let onboarded = state::with(|t| Ok(t.settings()?.onboarded)).unwrap_or(false);
                    if onboarded {
                        c.say("Bloco excluído. O tempo volta a ficar disponível a partir do próximo dia.");
                    }
                }
            }
        }
    };
    view! {
        <Sheet title=title>
            <p class="muted">{text}</p>
            <div class="row gap">
                <button type="button" class="btn grow" on:click=move |_| c.dialog.set(None)>"Cancelar"</button>
                <button type="button" class="btn danger grow" autofocus on:click=accept>{button}</button>
            </div>
        </Sheet>
    }
}
