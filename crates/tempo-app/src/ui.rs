//! Liga os callbacks da interface ao serviço e mantém a tela atualizada.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use chrono::Timelike;

use slint::{ComponentHandle, Model, SharedString, Timer, TimerMode};
use tempo_core::format::{hm, short};
use tempo_core::sleep::sleep_secs_between;
use tempo_core::tags::SUGGESTED_TAGS;
use tempo_core::{allocated_secs, Clock, ClockTime, Permille, SleepSchedule, SECS_PER_DAY};
use tempo_store::{NewTag, Tempo};

use crate::platform::{timer_sync, Platform};
use crate::present::{self, model};
use crate::{state, Api, AppWindow, Dlg, Suggestion, TagItem};

const RETENTION_OPTIONS: [Option<u32>; 4] = [None, Some(90), Some(180), Some(365)];

/// Dados em cache para os callbacks puros (avaliados durante a renderização).
#[derive(Default)]
struct Cache {
    available: Cell<i64>,
    active_tag_ids: RefCell<Vec<i64>>,
    plan_total_others: RefCell<Vec<(i64, u16)>>,
}

struct Ctx {
    ui: slint::Weak<AppWindow>,
    platform: Rc<dyn Platform>,
    cache: Cache,
    ticks: Cell<u64>,
}

/// Mantém vivos o timer de atualização e o contexto enquanto a janela existir.
pub struct Handle {
    _timer: Timer,
    _ctx: Rc<Ctx>,
}

impl Ctx {
    fn ui(&self) -> Option<AppWindow> {
        self.ui.upgrade()
    }

    fn toast(&self, msg: impl Into<SharedString>) {
        if let Some(ui) = self.ui() {
            ui.global::<Api>().set_toast(msg.into());
        }
    }

    /// Executa uma operação no serviço; erros viram aviso na tela.
    fn run<R>(&self, f: impl FnOnce(&mut Tempo) -> tempo_store::Result<R>) -> Option<R> {
        match state::with(f) {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("{e}");
                self.toast(e.to_string());
                None
            }
        }
    }

    /// Envia ao sistema o estado atual do timer (notificação + alarme).
    fn sync_platform(&self) {
        match state::with(|t| timer_sync(t)) {
            Ok(s) => self.platform.sync_timer(s.as_ref()),
            Err(e) => log::warn!("sync: {e}"),
        }
    }

    fn refresh_all(&self) {
        let Some(ui) = self.ui() else { return };
        let api = ui.global::<Api>();
        let onboarded = state::with(|t| Ok(t.settings()?.onboarded)).unwrap_or(false);
        api.set_onboarded(onboarded);
        if !onboarded {
            reset_suggestions(&ui);
            return;
        }
        self.refresh_today();
        self.refresh_plan();
        self.refresh_settings();
        if api.get_page() == 2 {
            self.refresh_history(api.get_history_tab());
        }
    }

    fn refresh_today(&self) {
        let Some(ui) = self.ui() else { return };
        let sessions_open = ui.global::<Dlg>().get_kind() == 3;
        let data = state::with(|t| {
            let report = t.tick()?;
            let view = t.today()?;
            let sessions = if sessions_open { Some(t.sessions_today()?) } else { None };
            Ok((report, view, sessions, t.clock().clone()))
        });
        let (report, view, sessions, clock) = match data {
            Ok(d) => d,
            Err(tempo_store::Error::NotOnboarded) => return,
            Err(e) => {
                log::warn!("today: {e}");
                return;
            }
        };
        if report.changed() {
            self.sync_platform();
        }
        if let Some(tag_id) = report.auto_stopped {
            if let Ok((name, s)) = state::with(|t| Ok((t.tag(tag_id)?.name, t.settings()?))) {
                self.platform.notify_finished(&name, s.sound, s.vibration);
                self.toast(format!("Tempo de \"{name}\" esgotado."));
            }
        }
        if report.day_rolled {
            self.refresh_plan();
        }

        self.cache.available.set(view.available);
        *self.cache.active_tag_ids.borrow_mut() = view.tags.iter().map(|t| t.tag.id).collect();

        let api = ui.global::<Api>();
        api.set_day(present::day_info(&view));
        api.set_segments(model(present::segments(&view)));
        api.set_hero(present::hero(&view, &*clock));
        api.set_today_tags(model(view.tags.iter().map(present::tag_item).collect()));
        api.set_tag_names(model(view.tags.iter().map(|t| SharedString::from(t.tag.name.as_str())).collect()));
        let local = clock.utc_to_local(view.now);
        let now_min = (local.hour() * 60 + local.minute()) as i32;
        api.set_manual_default_start((now_min - 30).rem_euclid(1440) / 5 * 5);
        if let Some(list) = sessions {
            api.set_sessions(model(list.iter().map(|s| present::session_item(s, &*clock, view.now)).collect()));
        }
    }

    fn refresh_sessions(&self) {
        let Some(ui) = self.ui() else { return };
        if let Some((list, clock, now)) = self.run(|t| Ok((t.sessions_today()?, t.clock().clone(), t.now()))) {
            ui.global::<Api>()
                .set_sessions(model(list.iter().map(|s| present::session_item(s, &*clock, now)).collect()));
        }
    }

    fn refresh_plan(&self) {
        let Some(ui) = self.ui() else { return };
        let Some((tags, available, settings)) =
            self.run(|t| Ok((t.tags(true)?, t.current_available()?, t.settings()?)))
        else {
            return;
        };
        self.cache.available.set(available);
        let active: Vec<_> = tags.iter().filter(|t| !t.archived).collect();
        *self.cache.plan_total_others.borrow_mut() = active.iter().map(|t| (t.id, t.permille.value())).collect();
        let total: u32 = active.iter().map(|t| u32::from(t.permille.value())).sum();
        let total = Permille::saturating(i64::from(total));
        let free = tempo_core::tags::unallocated(total);

        let api = ui.global::<Api>();
        api.set_plan_tags(model(
            active.iter().map(|t| present::plan_item(t.id, &t.name, t.color, t.permille, available)).collect(),
        ));
        api.set_archived_tags(model(
            tags.iter()
                .filter(|t| t.archived)
                .map(|t| present::plan_item(t.id, &t.name, t.color, t.permille, available))
                .collect::<Vec<TagItem>>(),
        ));
        api.set_plan_available(hm(available).into());
        let s = settings.schedule;
        api.set_plan_sleep(format!("Sono das {} às {} ({}).", s.bedtime(), s.wake(), short(s.sleep_secs())).into());
        api.set_plan_allocated(format!("Alocado {total}").into());
        api.set_plan_unallocated(
            format!("Não alocado {free} · {}", short(available - allocated_secs(available, total))).into(),
        );
        api.set_can_add_tag(active.len() < tempo_core::tags::MAX_ACTIVE_TAGS && free.value() >= 1);
    }

    fn refresh_settings(&self) {
        let Some(ui) = self.ui() else { return };
        let Some(s) = self.run(|t| t.settings()) else { return };
        let api = ui.global::<Api>();
        api.set_set_bed(i32::from(s.schedule.bedtime().minutes()));
        api.set_set_wake(i32::from(s.schedule.wake().minutes()));
        api.set_overtime(s.overtime);
        api.set_sound(s.sound);
        api.set_vibration(s.vibration);
        api.set_retention_index(RETENTION_OPTIONS.iter().position(|o| *o == s.retention_days).unwrap_or(0) as i32);
        self.refresh_permissions();
    }

    fn refresh_permissions(&self) {
        let Some(ui) = self.ui() else { return };
        let api = ui.global::<Api>();
        api.set_notif_granted(self.platform.notifications_allowed());
        api.set_exact_granted(self.platform.exact_alarms_allowed());
    }

    fn refresh_history(&self, tab: i32) {
        let Some(ui) = self.ui() else { return };
        let api = ui.global::<Api>();
        if tab == 0 {
            if let Some(days) = self.run(|t| {
                t.tick()?;
                t.history(60)
            }) {
                api.set_history(model(days.iter().map(present::history_day).collect()));
            }
        } else {
            let period = if tab == 1 { 7 } else { 30 };
            if let Some(stats) = self.run(|t| {
                t.tick()?;
                t.stats(period)
            }) {
                api.set_stats(model(
                    stats.tags.iter().map(|s| present::stat_item(&s.name, s.color, s.archived, &s.stats)).collect(),
                ));
                api.set_stats_caption(present::stats_caption(&stats));
            }
        }
    }

    /// Após uma ação no timer: sincroniza o sistema e atualiza a tela.
    fn after_timer_action(&self) {
        self.sync_platform();
        self.refresh_today();
    }

    fn max_permille_for(&self, id: i64) -> i32 {
        let others: u32 =
            self.cache.plan_total_others.borrow().iter().filter(|(t, _)| *t != id).map(|(_, p)| u32::from(*p)).sum();
        1000 - others.min(1000) as i32
    }
}

fn reset_suggestions(ui: &AppWindow) {
    let items: Vec<Suggestion> = SUGGESTED_TAGS
        .iter()
        .map(|&(name, p, c)| Suggestion {
            name: name.into(),
            color: present::color(c),
            permille: i32::from(p),
            percent: Permille::new(p).map(|p| p.to_string()).unwrap_or_default().into(),
            checked: true,
        })
        .collect();
    ui.global::<Api>().set_suggestions(model(items));
}

fn clock_time(minutes: i32) -> ClockTime {
    ClockTime::from_minutes(minutes.rem_euclid(1440) as u16).expect("minutos válidos")
}

fn schedule(bed: i32, wake: i32) -> Result<SleepSchedule, tempo_core::CoreError> {
    SleepSchedule::new(clock_time(bed), clock_time(wake))
}

pub fn setup(ui: &AppWindow, platform: Rc<dyn Platform>) -> Handle {
    let ctx = Rc::new(Ctx { ui: ui.as_weak(), platform, cache: Cache::default(), ticks: Cell::new(0) });
    let api = ui.global::<Api>();
    api.set_is_android(ctx.platform.is_android());
    api.set_version(env!("CARGO_PKG_VERSION").into());

    // ---------------------------------------------------------------- formatação pura
    api.on_clock_text(|m| clock_time(m).to_string().into());
    api.on_wrap_minutes(|m| m.rem_euclid(1440));
    api.on_sleep_valid(|b, w| schedule(b, w).is_ok());
    api.on_sleep_summary(|b, w| {
        let sleep = sleep_secs_between(clock_time(b), clock_time(w));
        if b == w {
            return "Escolha horários diferentes.".into();
        }
        format!("{} de sono · {} disponíveis por dia", short(sleep), short(SECS_PER_DAY - sleep)).into()
    });
    api.on_sleep_warning(|b, w| match schedule(b, w) {
        Err(e) => e.to_string().into(),
        Ok(s) => s.warning().map(|w| w.message()).unwrap_or("").into(),
    });
    api.on_permille_text(|p| Permille::saturating(i64::from(p)).to_string().into());
    {
        let c = ctx.clone();
        api.on_permille_duration(move |p| {
            short(allocated_secs(c.cache.available.get(), Permille::saturating(i64::from(p)))).into()
        });
    }
    {
        let c = ctx.clone();
        api.on_permille_minutes(move |p| {
            (allocated_secs(c.cache.available.get(), Permille::saturating(i64::from(p))) / 60) as i32
        });
    }
    {
        let c = ctx.clone();
        api.on_minutes_to_permille(move |m| {
            if m <= 0 {
                return 0;
            }
            match Permille::from_duration(i64::from(m) * 60, c.cache.available.get()) {
                Ok(p) => i32::from(p.value()),
                Err(_) => 1001,
            }
        });
    }
    api.on_minutes_text(|m| short(i64::from(m) * 60).into());
    {
        let c = ctx.clone();
        api.on_max_permille_for(move |id| c.max_permille_for(i64::from(id)));
    }

    // ---------------------------------------------------------------- navegação
    {
        let c = ctx.clone();
        api.on_navigate(move |page| {
            let Some(ui) = c.ui() else { return };
            let api = ui.global::<Api>();
            api.set_page(page);
            match page {
                0 => c.refresh_today(),
                1 => c.refresh_plan(),
                2 => c.refresh_history(api.get_history_tab()),
                3 => c.refresh_settings(),
                _ => {}
            }
        });
    }

    // ---------------------------------------------------------------- primeira execução
    {
        let c = ctx.clone();
        api.on_finish_onboarding(move |bed, wake| {
            let Some(ui) = c.ui() else { return };
            let s = match schedule(bed, wake) {
                Ok(s) => s,
                Err(e) => return c.toast(e.to_string()),
            };
            let chosen: Vec<NewTag> = ui
                .global::<Api>()
                .get_suggestions()
                .iter()
                .filter(|s| s.checked)
                .filter_map(|s| {
                    Some(NewTag {
                        name: s.name.to_string(),
                        color: present::color_u32(s.color),
                        permille: Permille::new(s.permille as u16).ok()?,
                    })
                })
                .collect();
            if c.run(|t| t.complete_onboarding(s, &chosen)).is_some() {
                c.platform.request_notifications();
                ui.global::<Api>().set_page(0);
                c.refresh_all();
            }
        });
    }

    // ---------------------------------------------------------------- timer
    {
        let c = ctx.clone();
        api.on_toggle_tag(move |id| {
            let id = i64::from(id);
            let running = state::with(|t| Ok(t.active_timer()?.is_some_and(|a| a.tag_id == id))).unwrap_or(false);
            let ok = if running { c.run(|t| t.pause()).is_some() } else { c.run(|t| t.start(id)).is_some() };
            if ok {
                c.after_timer_action();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_pause(move || {
            if c.run(|t| t.pause()).is_some() {
                c.after_timer_action();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_resume(move || {
            let Some(ui) = c.ui() else { return };
            let id = i64::from(ui.global::<Api>().get_hero().tag_id);
            if c.run(|t| t.start(id)).is_some() {
                c.after_timer_action();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_stop(move || {
            if c.run(|t| t.stop()).is_some() {
                c.after_timer_action();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_dismiss_clock_warning(move || {
            c.run(|t| t.dismiss_clock_warning());
            c.refresh_today();
        });
    }
    {
        let c = ctx.clone();
        api.on_open_sessions(move || c.refresh_sessions());
    }
    {
        let c = ctx.clone();
        api.on_delete_session(move |id| {
            if c.run(|t| t.delete_session(i64::from(id))).is_some() {
                c.refresh_sessions();
                c.refresh_today();
                c.toast("Sessão apagada.");
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_add_manual(move |tag_index, start_min, minutes| {
            let Some(&tag_id) = c.cache.active_tag_ids.borrow().get(tag_index.max(0) as usize) else {
                c.toast("Escolha uma tag.");
                return false;
            };
            let ok = c
                .run(|t| {
                    let start = t.day_time_to_utc(clock_time(start_min))?;
                    t.add_manual(tag_id, start, i64::from(minutes) * 60)
                })
                .is_some();
            if ok {
                c.refresh_today();
                c.toast("Tempo lançado.");
            }
            ok
        });
    }

    // ---------------------------------------------------------------- planejamento
    {
        let c = ctx.clone();
        api.on_save_tag(move |id, name, color, permille| {
            let permille = match Permille::new(permille.clamp(0, 1000) as u16) {
                Ok(p) => p,
                Err(e) => {
                    c.toast(e.to_string());
                    return false;
                }
            };
            let new = NewTag { name: name.to_string(), color: present::color_u32(color), permille };
            let ok = if id < 0 {
                c.run(|t| t.create_tag(&new)).is_some()
            } else {
                c.run(|t| t.update_tag(i64::from(id), &new)).is_some()
            };
            if ok {
                c.sync_platform();
                c.refresh_plan();
                c.refresh_today();
            }
            ok
        });
    }
    {
        let c = ctx.clone();
        api.on_move_tag(move |id, dir| {
            if c.run(|t| t.move_tag(i64::from(id), dir)).is_some() {
                c.refresh_plan();
                c.refresh_today();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_archive_tag(move |id| {
            if c.run(|t| t.archive_tag(i64::from(id))).is_some() {
                c.sync_platform();
                c.refresh_plan();
                c.refresh_today();
                c.toast("Tag arquivada. O histórico foi mantido.");
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_restore_tag(move |id| {
            if c.run(|t| t.restore_tag(i64::from(id))).is_some() {
                c.refresh_plan();
                c.refresh_today();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_confirm_accepted(move |action| {
            if action == "erase" {
                if c.run(|t| t.erase_all()).is_some() {
                    c.platform.sync_timer(None);
                    if let Some(ui) = c.ui() {
                        ui.global::<Api>().set_page(0);
                    }
                    c.refresh_all();
                }
            } else if let Some(id) = action.strip_prefix("delete-tag:").and_then(|v| v.parse::<i64>().ok()) {
                if c.run(|t| t.delete_tag(id)).is_some() {
                    c.sync_platform();
                    c.refresh_plan();
                    c.refresh_today();
                    c.toast("Tag excluída.");
                }
            }
        });
    }

    // ---------------------------------------------------------------- histórico
    {
        let c = ctx.clone();
        api.on_load_history(move |tab| c.refresh_history(tab));
    }

    // ---------------------------------------------------------------- ajustes
    {
        let c = ctx.clone();
        api.on_save_schedule(move |bed, wake| match schedule(bed, wake) {
            Ok(s) => {
                if c.run(|t| t.set_schedule(s)).is_some() {
                    c.refresh_plan();
                    c.toast("Horários salvos. Valem a partir do próximo dia.");
                }
            }
            Err(e) => c.toast(e.to_string()),
        });
    }
    {
        let c = ctx.clone();
        api.on_set_overtime(move |v| {
            if c.run(|t| t.set_overtime(v)).is_some() {
                c.sync_platform();
                c.refresh_settings();
                c.refresh_today();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_set_sound(move |v| {
            if c.run(|t| t.set_sound(v)).is_some() {
                c.sync_platform();
                c.refresh_settings();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_set_vibration(move |v| {
            if c.run(|t| t.set_vibration(v)).is_some() {
                c.sync_platform();
                c.refresh_settings();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_set_retention(move |i| {
            let days = RETENTION_OPTIONS.get(i.max(0) as usize).copied().flatten();
            if c.run(|t| t.set_retention(days)).is_some() {
                c.refresh_settings();
            }
        });
    }
    {
        let c = ctx.clone();
        api.on_request_notifications(move || c.platform.request_notifications());
    }
    {
        let c = ctx.clone();
        api.on_request_exact_alarms(move || c.platform.request_exact_alarms());
    }
    {
        let c = ctx.clone();
        api.on_export_csv(move || {
            if let Some(csv) = c.run(|t| t.export_csv()) {
                match c.platform.share_text("Tempo — sessões", &csv) {
                    Ok(msg) | Err(msg) => {
                        if !msg.is_empty() {
                            c.toast(msg);
                        }
                    }
                }
            }
        });
    }

    // ---------------------------------------------------------------- estado inicial
    reset_suggestions(ui);
    ctx.refresh_all();
    ctx.sync_platform();

    // Atualiza a cada segundo: timer, virada do dia e permissões (a cada 3 s na aba Ajustes).
    let timer = Timer::default();
    {
        let c = ctx.clone();
        timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let n = c.ticks.get() + 1;
            c.ticks.set(n);
            c.refresh_today();
            if n % 3 == 0 && c.ui().is_some_and(|ui| ui.global::<Api>().get_page() == 3) {
                c.refresh_permissions();
            }
        });
    }
    Handle { _timer: timer, _ctx: ctx }
}
