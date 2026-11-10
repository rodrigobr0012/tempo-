//! Planejamento: tempo reservado (RF01–RF03) e tags com porcentagens (RF06–RF13).

use leptos::prelude::*;
use tempo_core::format::{hm, short};
use tempo_core::tags::{unallocated, MAX_ACTIVE_TAGS};
use tempo_core::{allocated_secs, available_secs, Permille, SECS_PER_DAY};
use tempo_store::Tag;

use crate::state::{ctx, Confirm, Dialog};
use crate::ui::{css, icon, Icon, IconButton};

#[derive(Clone, Debug, Default, PartialEq)]
struct PlanData {
    active: Vec<Tag>,
    archived: Vec<Tag>,
    available: i64,
}

impl PlanData {
    fn total(&self) -> Permille {
        tempo_core::tags::total(self.active.iter().map(|t| t.permille))
    }
}

#[component]
pub fn PlanPage() -> impl IntoView {
    let c = ctx();
    let data = Memo::new(move |_| {
        c.read(|t| {
            let (archived, active) = t.tags(true)?.into_iter().partition(|t| t.archived);
            Ok(PlanData { active, archived, available: t.current_available()? })
        })
        .unwrap_or_default()
    });
    let can_add = move || data.with(|d| d.active.len() < MAX_ACTIVE_TAGS && unallocated(d.total()).value() >= 1);

    view! {
        <header class="page-head">
            <h1 class="grow">"Planejar"</h1>
        </header>

        <div class="plan-grid">
            <div class="stack">
                <section class="card">
                    <div class="row baseline">
                        <span class="big-number grow">{move || data.with(|d| hm(d.available))}</span>
                        <span class="muted">"por dia"</span>
                    </div>
                    <p class="muted">
                        {move || {
                            let reserved = SECS_PER_DAY - data.with(|d| d.available);
                            if reserved > 0 {
                                format!("Hoje: 24h − {} reservadas.", short(reserved))
                            } else {
                                "Hoje: 24h inteiras, sem tempo reservado.".to_string()
                            }
                        }}
                    </p>
                    <div class="stacked" role="img" aria-label="Divisão planejada">
                        {move || data.with(|d| {
                            d.active
                                .iter()
                                .map(|t| view! {
                                    <span
                                        style:background=css(t.color)
                                        style:width=format!("{:.2}%", t.permille.fraction() * 100.0)
                                    ></span>
                                })
                                .collect_view()
                        })}
                    </div>
                    <div class="row baseline small">
                        <span class="grow">
                            "Alocado "
                            <span class="pct lg">{move || data.with(|d| d.total().to_string())}</span>
                        </span>
                        <span class="muted">
                            "Não alocado "
                            <span class="pct sm">{move || data.with(|d| unallocated(d.total()).to_string())}</span>
                            " · "
                            {move || data.with(|d| {
                                short(d.available - allocated_secs(d.available, d.total()))
                            })}
                        </span>
                    </div>
                </section>

                <h2 class="section">"Tempo reservado"</h2>
                <p class="muted">
                    "Blocos que saem das 24h antes da divisão: sono, faculdade, deslocamento… Mudanças valem a partir do próximo dia."
                </p>
                <ReservedList />
            </div>

            <div class="stack">
                <h2 class="section">"Tags"</h2>
                {move || {
                    let d = data.get();
                    let n = d.active.len();
                    d.active
                        .into_iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let id = t.id;
                            let name = t.name.clone();
                            view! {
                                <div class="tag-row plan">
                                    <button
                                        type="button"
                                        class="plan-edit"
                                        aria-label=format!("Editar {}", t.name)
                                        on:click=move |_| c.dialog.set(Some(Dialog::TagEditor { id: Some(id) }))
                                    >
                                        <span class="dot big" style:background=css(t.color)></span>
                                        <span class="tag-body">
                                            <span class="row baseline">
                                                <span class="tag-name grow ellipsis">{t.name.clone()}</span>
                                                <span class="pct">{t.permille.to_string()}</span>
                                            </span>
                                            <span class="muted small">
                                                {format!("{} por dia", hm(allocated_secs(d.available, t.permille)))}
                                            </span>
                                        </span>
                                        <span class="muted"><Icon path=icon::EDIT /></span>
                                    </button>
                                    <IconButton
                                        path=icon::UP
                                        label=format!("Mover {name} para cima")
                                        disabled=i == 0
                                        on_click=move || { c.act(|t| t.move_tag(id, -1)); }
                                    />
                                    <IconButton
                                        path=icon::DOWN
                                        label=format!("Mover {name} para baixo")
                                        disabled=i + 1 == n
                                        on_click=move || { c.act(|t| t.move_tag(id, 1)); }
                                    />
                                </div>
                            }
                        })
                        .collect_view()
                }}

                <button
                    type="button"
                    class="btn primary"
                    disabled=move || !can_add()
                    on:click=move |_| c.dialog.set(Some(Dialog::TagEditor { id: None }))
                >
                    <Icon path=icon::ADD />
                    "Nova tag"
                </button>
                <Show when=move || !can_add()>
                    <p class="muted center small">
                        "Para criar outra tag, reduza alguma porcentagem (limite de 100%) ou arquive uma tag (limite de 20 tags)."
                    </p>
                </Show>

                <Show when=move || data.with(|d| !d.archived.is_empty())>
                    <h2 class="section">"Arquivadas"</h2>
                </Show>
                {move || data.with(|d| {
                    d.archived
                        .iter()
                        .map(|t| {
                            let id = t.id;
                            let name = t.name.clone();
                            let name2 = t.name.clone();
                            view! {
                                <div class="tag-row archived">
                                    <span class="dot big faded" style:background=css(t.color)></span>
                                    <span class="grow muted ellipsis">{t.name.clone()}</span>
                                    <span class="pct sm muted">{t.permille.to_string()}</span>
                                    <IconButton
                                        path=icon::UNARCHIVE
                                        label=format!("Restaurar {name}")
                                        on_click=move || { c.act(|t| t.restore_tag(id)); }
                                    />
                                    <IconButton
                                        path=icon::DELETE
                                        label=format!("Excluir {name}")
                                        danger=true
                                        on_click=move || c.confirm(
                                            format!("Excluir \"{name2}\"?"),
                                            "A tag e todo o histórico dela serão apagados. Isso não pode ser desfeito.",
                                            "Excluir",
                                            Confirm::DeleteTag(id),
                                        )
                                    />
                                </div>
                            }
                        })
                        .collect_view()
                })}
            </div>
        </div>
    }
}

/// Lista de blocos reservados com botão de adicionar (Planejar e primeira execução).
#[component]
pub fn ReservedList() -> impl IntoView {
    let c = ctx();
    let list = Memo::new(move |_| c.read(|t| t.reserved()).unwrap_or_default());
    view! {
        <div class="stack">
            <Show when=move || list.with(Vec::is_empty)>
                <p class="muted center">"Nenhum bloco: as 24h inteiras ficam disponíveis para as tags."</p>
            </Show>
            {move || list.with(|l| {
                l.iter()
                    .map(|r| {
                        let id = r.id;
                        view! {
                            <button
                                type="button"
                                class="tag-row"
                                aria-label=format!("Editar {}, {}", r.name, short(r.duration))
                                on:click=move |_| c.dialog.set(Some(Dialog::Reserved { id: Some(id) }))
                            >
                                <span class="stripe neutral"></span>
                                <span class="tag-name grow ellipsis">{r.name.clone()}</span>
                                <span class="muted">{short(r.duration)}</span>
                                <span class="muted"><Icon path=icon::EDIT /></span>
                            </button>
                        }
                    })
                    .collect_view()
            })}
            <button type="button" class="btn" on:click=move |_| c.dialog.set(Some(Dialog::Reserved { id: None }))>
                <Icon path=icon::ADD />
                "Novo bloco reservado"
            </button>
        </div>
    }
}

/// "8h reservadas · 16h disponíveis por dia"
pub fn reserved_summary(total: i64) -> String {
    format!("{} reservadas · {} disponíveis por dia", short(total), short(available_secs(total)))
}
