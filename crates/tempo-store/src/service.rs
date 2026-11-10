//! Serviço de aplicação: aplica as regras do `tempo-core` sobre o banco SQLite.
//!
//! Toda operação que altera mais de uma linha roda em transação (RNF10).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use tempo_core::day::{day_bounds_containing, day_bounds_for_date};
use tempo_core::stats::{aggregate, DayTagRecord};
use tempo_core::tags::{check_allocation, name_key, normalize_name, total, unallocated, MAX_ACTIVE_TAGS};
use tempo_core::timer::{deadline, Progress};
use tempo_core::{allocated_secs, format, Clock, ClockTime, CoreError, Permille, SleepSchedule};

use crate::error::{Error, Result};
use crate::model::*;
use crate::schema;

/// Tolerância antes de considerar que o relógio voltou (RN24).
const CLOCK_BACK_TOLERANCE: i64 = 120;
/// Intervalo mínimo entre gravações de `last_seen`, para não escrever no disco a cada segundo.
const LAST_SEEN_WRITE_EVERY: i64 = 30;
/// Ausência máxima preenchida dia a dia (RN27). Acima disso, pula direto para hoje.
const MAX_GAP_DAYS: i64 = 400;

mod key {
    pub const BEDTIME: &str = "bedtime";
    pub const WAKE: &str = "wake";
    pub const OVERTIME: &str = "overtime";
    pub const SOUND: &str = "sound";
    pub const VIBRATION: &str = "vibration";
    pub const RETENTION: &str = "retention_days";
    pub const ONBOARDED: &str = "onboarded";
    pub const FOCUS: &str = "focus_tag";
    pub const LAST_SEEN: &str = "last_seen";
    pub const CLOCK_WARNING: &str = "clock_warning";
}

pub struct Tempo {
    conn: Connection,
    clock: Arc<dyn Clock>,
}

#[derive(Clone, Copy, Debug)]
struct DayRow {
    id: i64,
    date: NaiveDate,
    start: i64,
    end: i64,
    available: i64,
}

#[derive(Clone, Copy, Debug)]
struct ActiveRow {
    id: i64,
    day_id: i64,
    tag_id: i64,
    start: i64,
}

impl Tempo {
    pub fn open(path: impl AsRef<Path>, clock: Arc<dyn Clock>) -> Result<Self> {
        Self::init(Connection::open(path)?, clock)
    }

    pub fn open_in_memory(clock: Arc<dyn Clock>) -> Result<Self> {
        Self::init(Connection::open_in_memory()?, clock)
    }

    fn init(mut conn: Connection, clock: Arc<dyn Clock>) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        schema::migrate(&mut conn)?;
        Ok(Self { conn, clock })
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    pub fn now(&self) -> i64 {
        self.clock.now()
    }

    // ------------------------------------------------------------------ configurações

    pub fn settings(&self) -> Result<Settings> {
        read_settings(&self.conn)
    }

    /// Primeira execução (RF04, RF14): grava o sono, cria as tags escolhidas e abre o dia.
    pub fn complete_onboarding(&mut self, schedule: SleepSchedule, tags: &[NewTag]) -> Result<()> {
        let now = self.now();
        let tx = self.conn.transaction()?;
        write_schedule(&tx, schedule)?;
        for t in tags {
            insert_tag(&tx, t, now)?;
        }
        set_kv(&tx, key::ONBOARDED, "1")?;
        tx.commit()?;
        self.tick()?;
        Ok(())
    }

    /// RN16: o novo sono vale a partir do próximo dia lógico.
    pub fn set_schedule(&mut self, schedule: SleepSchedule) -> Result<()> {
        write_schedule(&self.conn, schedule)
    }

    pub fn set_overtime(&mut self, on: bool) -> Result<()> {
        self.tick()?;
        let now = self.now();
        let tx = self.conn.transaction()?;
        if !on {
            // Desligar com o timer já no excedente: encerra agora, sem apagar o tempo já contado.
            if let (Some(a), Some(day)) = (active_session(&tx)?, open_day(&tx)?) {
                if tag_progress(&tx, &day, a.tag_id, now)?.is_finished() {
                    close_session(&tx, a.id, now)?;
                }
            }
        }
        set_bool(&tx, key::OVERTIME, on)?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_sound(&mut self, on: bool) -> Result<()> {
        set_bool(&self.conn, key::SOUND, on)
    }

    pub fn set_vibration(&mut self, on: bool) -> Result<()> {
        set_bool(&self.conn, key::VIBRATION, on)
    }

    /// RN30: `None` mantém o histórico para sempre.
    pub fn set_retention(&mut self, days: Option<u32>) -> Result<()> {
        set_kv(&self.conn, key::RETENTION, &days.unwrap_or(0).to_string())?;
        let tx = self.conn.transaction()?;
        if let Some(day) = open_day(&tx)? {
            apply_retention(&tx, days, day.date)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Descarta o aviso de relógio alterado (RN24).
    pub fn dismiss_clock_warning(&mut self) -> Result<()> {
        del_kv(&self.conn, key::CLOCK_WARNING)
    }

    /// RF36: apaga tudo e volta para a primeira execução.
    pub fn erase_all(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "DELETE FROM sessions; DELETE FROM day_allocations; DELETE FROM days;
             DELETE FROM tags; DELETE FROM settings;",
        )?;
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------ ciclo do dia

    /// Mantém o estado em dia: verifica o relógio (RN24), encerra timers esgotados (RN20),
    /// vira o dia lógico (RN21, RN26, RN27) e aplica a retenção (RN30).
    /// Deve ser chamado ao abrir o app, periodicamente e antes de qualquer consulta.
    pub fn tick(&mut self) -> Result<TickReport> {
        let s = self.settings()?;
        let mut report = TickReport::default();
        if !s.onboarded {
            return Ok(report);
        }
        let now = self.now();
        let clock = self.clock.clone();
        let tx = self.conn.transaction()?;

        check_clock(&tx, now, &mut report)?;

        let mut day = match open_day(&tx)? {
            Some(d) => d,
            None => {
                report.day_rolled = true;
                create_day_containing(&tx, &*clock, now, &s)?
            }
        };

        if now - day.end > MAX_GAP_DAYS * 86_400 {
            if let Some(a) = active_session(&tx)? {
                close_session(&tx, a.id, day.end)?;
            }
            close_day(&tx, day.id)?;
            day = create_day_containing(&tx, &*clock, now, &s)?;
            report.day_rolled = true;
        }

        loop {
            if !s.overtime {
                if let Some(tag) = auto_stop(&tx, &day, now.min(day.end))? {
                    report.auto_stopped = Some(tag);
                }
            }
            if now < day.end {
                break;
            }
            day = roll_over(&tx, &*clock, &day, &s)?;
            report.day_rolled = true;
        }

        if report.day_rolled {
            apply_retention(&tx, s.retention_days, day.date)?;
        }
        tx.commit()?;
        Ok(report)
    }

    /// TD do dia atual (ou do sono configurado, se ainda não há dia aberto).
    pub fn current_available(&self) -> Result<i64> {
        match open_day(&self.conn)? {
            Some(d) => Ok(d.available),
            None => Ok(self.settings()?.schedule.available_secs()),
        }
    }

    /// Converte um horário do dia lógico atual para instante (o dia lógico cruza a meia-noite).
    pub fn day_time_to_utc(&self, t: ClockTime) -> Result<i64> {
        let day = open_day(&self.conn)?.ok_or(Error::NotOnboarded)?;
        let first = self.clock.local_to_utc(day.date.and_time(t.to_naive_time()));
        if first >= day.start {
            return Ok(first);
        }
        let next = day.date.succ_opt().unwrap_or(day.date);
        Ok(self.clock.local_to_utc(next.and_time(t.to_naive_time())))
    }

    // ------------------------------------------------------------------ tags

    /// Tags ordenadas pela posição. `include_archived` inclui as arquivadas no fim.
    pub fn tags(&self, include_archived: bool) -> Result<Vec<Tag>> {
        let sql = if include_archived {
            "SELECT id, name, color, permille, position, archived FROM tags ORDER BY archived, position"
        } else {
            "SELECT id, name, color, permille, position, archived FROM tags WHERE archived = 0 ORDER BY position"
        };
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map([], tag_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn tag(&self, id: i64) -> Result<Tag> {
        get_tag(&self.conn, id)
    }

    pub fn create_tag(&mut self, new: &NewTag) -> Result<i64> {
        self.tick()?;
        let now = self.now();
        let tx = self.conn.transaction()?;
        let id = insert_tag(&tx, new, now)?;
        sync_today_allocation(&tx, id)?;
        tx.commit()?;
        Ok(id)
    }

    /// RN15: a nova porcentagem vale imediatamente; o consumido é preservado.
    pub fn update_tag(&mut self, id: i64, new: &NewTag) -> Result<()> {
        self.tick()?;
        let now = self.now();
        let s = self.settings()?;
        let tx = self.conn.transaction()?;
        let tag = get_tag(&tx, id)?;
        let name = normalize_name(&new.name)?;
        let key = name_key(&name);
        if !tag.archived {
            ensure_unique_name(&tx, &key, &name, Some(id))?;
            check_allocation(active_total(&tx, Some(id))?, Permille::new(new.permille.value())?)?;
        }
        tx.execute(
            "UPDATE tags SET name = ?1, name_key = ?2, color = ?3, permille = ?4 WHERE id = ?5",
            params![name, key, new.color, new.permille.value(), id],
        )?;
        if !tag.archived {
            // Sem excedente: se a redução esgota o timer em andamento, encerra agora (não no passado).
            if !s.overtime {
                if let (Some(a), Some(day)) = (active_session(&tx)?, open_day(&tx)?) {
                    if a.tag_id == id {
                        let consumed = tag_progress(&tx, &day, id, now)?.consumed;
                        if consumed >= allocated_secs(day.available, new.permille) {
                            close_session(&tx, a.id, now)?;
                        }
                    }
                }
            }
            sync_today_allocation(&tx, id)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// RN14: arquivar mantém o histórico e libera a porcentagem.
    pub fn archive_tag(&mut self, id: i64) -> Result<()> {
        self.tick()?;
        let now = self.now();
        let tx = self.conn.transaction()?;
        get_tag(&tx, id)?;
        if let Some(a) = active_session(&tx)? {
            if a.tag_id == id {
                close_session(&tx, a.id, now)?;
            }
        }
        tx.execute("UPDATE tags SET archived = 1 WHERE id = ?1", [id])?;
        if focus_tag(&tx)? == Some(id) {
            del_kv(&tx, key::FOCUS)?;
        }
        if let Some(day) = open_day(&tx)? {
            let has_sessions: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE day_id = ?1 AND tag_id = ?2)",
                [day.id, id],
                |r| r.get(0),
            )?;
            if has_sessions {
                tx.execute(
                    "UPDATE day_allocations SET permille = 0, allocated_secs = 0 WHERE day_id = ?1 AND tag_id = ?2",
                    [day.id, id],
                )?;
            } else {
                tx.execute("DELETE FROM day_allocations WHERE day_id = ?1 AND tag_id = ?2", [day.id, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Restaura uma tag arquivada com a porcentagem que tinha, se couber.
    pub fn restore_tag(&mut self, id: i64) -> Result<()> {
        self.tick()?;
        let tx = self.conn.transaction()?;
        let tag = get_tag(&tx, id)?;
        if !tag.archived {
            return Ok(());
        }
        if active_count(&tx)? >= MAX_ACTIVE_TAGS {
            return Err(CoreError::TooManyTags.into());
        }
        ensure_unique_name(&tx, &name_key(&tag.name), &tag.name, None)?;
        let p = Permille::new(tag.permille.value().max(1))?;
        if check_allocation(active_total(&tx, None)?, p).is_err() {
            return Err(Error::NoRoomToRestore(tag.name));
        }
        let pos = next_position(&tx)?;
        tx.execute(
            "UPDATE tags SET archived = 0, permille = ?1, position = ?2 WHERE id = ?3",
            params![p.value(), pos, id],
        )?;
        sync_today_allocation(&tx, id)?;
        tx.commit()?;
        Ok(())
    }

    /// RN14: excluir apaga a tag e todo o seu histórico.
    pub fn delete_tag(&mut self, id: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        get_tag(&tx, id)?;
        if focus_tag(&tx)? == Some(id) {
            del_kv(&tx, key::FOCUS)?;
        }
        tx.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// RF12: move a tag uma posição para cima (`-1`) ou para baixo (`+1`).
    pub fn move_tag(&mut self, id: i64, direction: i32) -> Result<()> {
        let tags = self.tags(false)?;
        let Some(i) = tags.iter().position(|t| t.id == id) else {
            return Err(Error::TagNotFound);
        };
        let j = i as i64 + i64::from(direction.signum());
        if j < 0 || j >= tags.len() as i64 {
            return Ok(());
        }
        let other = &tags[j as usize];
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE tags SET position = ?1 WHERE id = ?2", params![other.position, id])?;
        tx.execute("UPDATE tags SET position = ?1 WHERE id = ?2", params![tags[i].position, other.id])?;
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------ timer

    /// Inicia (ou retoma) o timer de uma tag. Pausa o timer que estiver rodando (RN17).
    pub fn start(&mut self, tag_id: i64) -> Result<()> {
        self.tick()?;
        let s = self.settings()?;
        let now = self.now();
        let tx = self.conn.transaction()?;
        let tag = get_tag(&tx, tag_id)?;
        if tag.archived {
            return Err(Error::TagNotFound);
        }
        let day = open_day(&tx)?.ok_or(Error::NotOnboarded)?;
        ensure_allocation_row(&tx, &day, &tag)?;
        if let Some(a) = active_session(&tx)? {
            if a.tag_id == tag_id {
                return Ok(());
            }
            close_session(&tx, a.id, now)?;
        }
        if !s.overtime && tag_progress(&tx, &day, tag_id, now)?.is_finished() {
            return Err(Error::TimeExhausted(tag.name));
        }
        tx.execute(
            "INSERT INTO sessions (day_id, tag_id, start_utc, end_utc, origin) VALUES (?1, ?2, ?3, NULL, 'timer')",
            params![day.id, tag_id, now],
        )?;
        set_kv(&tx, key::FOCUS, &tag_id.to_string())?;
        tx.commit()?;
        Ok(())
    }

    /// Pausa: encerra a sessão atual e mantém a tag no cartão principal (RN19).
    pub fn pause(&mut self) -> Result<bool> {
        self.tick()?;
        let now = self.now();
        let tx = self.conn.transaction()?;
        let changed = match active_session(&tx)? {
            Some(a) => {
                close_session(&tx, a.id, now)?;
                true
            }
            None => false,
        };
        tx.commit()?;
        Ok(changed)
    }

    /// Para: encerra a sessão e tira a tag do cartão principal (RN19).
    pub fn stop(&mut self) -> Result<bool> {
        let changed = self.pause()?;
        del_kv(&self.conn, key::FOCUS)?;
        Ok(changed)
    }

    /// Timer em andamento, se houver.
    pub fn active_timer(&self) -> Result<Option<ActiveTimer>> {
        let Some(day) = open_day(&self.conn)? else { return Ok(None) };
        active_timer(&self.conn, &day)
    }

    /// RF23/RN23: lançamento manual no dia atual.
    pub fn add_manual(&mut self, tag_id: i64, start: i64, duration_secs: i64) -> Result<i64> {
        self.tick()?;
        if duration_secs <= 0 {
            return Err(CoreError::DurationZero.into());
        }
        let now = self.now();
        let end = start + duration_secs;
        let tx = self.conn.transaction()?;
        let tag = get_tag(&tx, tag_id)?;
        if tag.archived {
            return Err(Error::TagNotFound);
        }
        let day = open_day(&tx)?.ok_or(Error::NotOnboarded)?;
        if end > now {
            return Err(CoreError::ManualInFuture.into());
        }
        if start < day.start {
            return Err(CoreError::ManualOutsideDay.into());
        }
        let new_span = tempo_core::Span::closed(start, end);
        for s in day_spans(&tx, day.id)? {
            if new_span.overlaps(&s, now) {
                return Err(CoreError::ManualOverlap.into());
            }
        }
        ensure_allocation_row(&tx, &day, &tag)?;
        tx.execute(
            "INSERT INTO sessions (day_id, tag_id, start_utc, end_utc, origin) VALUES (?1, ?2, ?3, ?4, 'manual')",
            params![day.id, tag_id, start, end],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// RF24: apaga uma sessão encerrada do dia atual.
    pub fn delete_session(&mut self, id: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let row: Option<(i64, Option<i64>)> = tx
            .query_row("SELECT day_id, end_utc FROM sessions WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let (day_id, end) = row.ok_or(Error::SessionNotFound)?;
        if end.is_none() {
            return Err(Error::SessionActive);
        }
        if open_day(&tx)?.map(|d| d.id) != Some(day_id) {
            return Err(Error::SessionNotToday);
        }
        tx.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------ consultas

    /// Painel do dia. Chame [`Tempo::tick`] antes.
    pub fn today(&self) -> Result<TodayView> {
        let conn = &self.conn;
        let day = open_day(conn)?.ok_or(Error::NotOnboarded)?;
        let now = self.now();
        let active = active_timer(conn, &day)?;

        let mut st = conn.prepare(
            "SELECT t.id, t.name, t.color, t.permille, t.position, t.archived,
                    COALESCE(a.permille, t.permille), a.allocated_secs
             FROM tags t
             LEFT JOIN day_allocations a ON a.tag_id = t.id AND a.day_id = ?1
             WHERE t.archived = 0
             ORDER BY t.position",
        )?;
        let consumed = consumed_by_tag(conn, day.id, now)?;
        let tags: Vec<TagToday> = st
            .query_map([day.id], |r| {
                let tag = tag_from_row(r)?;
                let permille = Permille::saturating(r.get(6)?);
                let allocated: Option<i64> = r.get(7)?;
                Ok((tag, permille, allocated))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(tag, permille, allocated)| {
                let allocated = allocated.unwrap_or_else(|| allocated_secs(day.available, permille));
                let consumed = consumed.iter().find(|(id, _)| *id == tag.id).map_or(0, |(_, c)| *c);
                let running = active.as_ref().is_some_and(|a| a.tag_id == tag.id);
                TagToday { tag, permille, allocated, consumed, running }
            })
            .collect();

        let allocated_permille = total(tags.iter().map(|t| t.permille));
        let allocated_sum: i64 = tags.iter().map(|t| t.allocated).sum();
        let awake_end = day.start + day.available;
        let mut view = TodayView {
            date: day.date,
            now,
            day_start: day.start,
            day_end: day.end,
            available: day.available,
            awake_elapsed: (now - day.start).max(0),
            awake_remaining: (awake_end - now).max(0),
            unallocated_permille: unallocated(allocated_permille),
            unallocated_secs: (day.available - allocated_sum).max(0),
            allocated_permille,
            tags,
            active,
            focus_tag: focus_tag(conn)?,
            overcommitted: false,
            clock_warning: get_kv(conn, key::CLOCK_WARNING)?.is_some(),
        };
        view.overcommitted = view.total_remaining() > view.awake_remaining;
        Ok(view)
    }

    /// Sessões do dia atual, da mais recente para a mais antiga.
    pub fn sessions_today(&self) -> Result<Vec<SessionView>> {
        let Some(day) = open_day(&self.conn)? else { return Ok(vec![]) };
        let mut st = self.conn.prepare(
            "SELECT s.id, s.tag_id, t.name, t.color, s.start_utc, s.end_utc, s.origin
             FROM sessions s JOIN tags t ON t.id = s.tag_id
             WHERE s.day_id = ?1 ORDER BY s.start_utc DESC",
        )?;
        let rows = st.query_map([day.id], |r| {
            Ok(SessionView {
                id: r.get(0)?,
                tag_id: r.get(1)?,
                tag_name: r.get(2)?,
                color: r.get(3)?,
                start: r.get(4)?,
                end: r.get(5)?,
                origin: Origin::parse(&r.get::<_, String>(6)?),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// RF30: histórico por dia, do mais recente para o mais antigo (inclui o dia atual).
    pub fn history(&self, limit: u32) -> Result<Vec<DaySummary>> {
        let now = self.now();
        let mut st = self
            .conn
            .prepare("SELECT id, date, available_secs, closed, no_record FROM days ORDER BY date DESC LIMIT ?1")?;
        let days = st
            .query_map([limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, bool>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        days.into_iter()
            .map(|(id, date, available, closed, no_record)| {
                Ok(DaySummary {
                    date: date_from_db(date),
                    available,
                    closed,
                    no_record,
                    tags: day_tag_summaries(&self.conn, id, now)?,
                })
            })
            .collect()
    }

    /// RF31: estatísticas dos últimos `period_days` dias completos (fechados).
    pub fn stats(&self, period_days: u32) -> Result<StatsView> {
        let Some(today) = open_day(&self.conn)? else { return Ok(StatsView::default()) };
        let from = date_to_db(today.date) - period_days as i32;
        let mut st =
            self.conn.prepare("SELECT id FROM days WHERE closed = 1 AND date >= ?1 AND date < ?2 ORDER BY date")?;
        let ids = st
            .query_map([from, date_to_db(today.date)], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let now = self.now();
        let mut days = Vec::with_capacity(ids.len());
        for id in &ids {
            days.push(
                day_tag_summaries(&self.conn, *id, now)?
                    .into_iter()
                    .map(|t| DayTagRecord { tag_id: t.tag_id, allocated: t.allocated, consumed: t.consumed })
                    .collect::<Vec<_>>(),
            );
        }
        let all_tags = self.tags(true)?;
        let mut tags: Vec<TagStatsView> = aggregate(&days)
            .into_iter()
            .filter_map(|stats| {
                let t = all_tags.iter().find(|t| t.id == stats.tag_id)?;
                Some(TagStatsView { tag_id: t.id, name: t.name.clone(), color: t.color, archived: t.archived, stats })
            })
            .collect();
        tags.sort_by_key(|t| (t.archived, all_tags.iter().position(|a| a.id == t.tag_id)));
        Ok(StatsView { days: ids.len() as u32, tags })
    }

    /// RF32: exporta todas as sessões em CSV (separador `;`, horários locais).
    pub fn export_csv(&self) -> Result<String> {
        let now = self.now();
        let mut st = self.conn.prepare(
            "SELECT d.date, t.name, s.start_utc, s.end_utc, s.origin
             FROM sessions s JOIN days d ON d.id = s.day_id JOIN tags t ON t.id = s.tag_id
             ORDER BY s.start_utc",
        )?;
        let mut out = String::from("dia;tag;inicio;fim;duracao_segundos;duracao;origem\n");
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (date, name, start, end, origin) = row?;
            let end_eff = end.unwrap_or(now);
            let dur = (end_eff - start).max(0);
            let fmt = |ts: i64| self.clock.utc_to_local(ts).format("%Y-%m-%d %H:%M:%S").to_string();
            out.push_str(&format!(
                "{};{};{};{};{};{};{}\n",
                date_from_db(date),
                csv_field(&name),
                fmt(start),
                end.map(fmt).unwrap_or_else(|| "em andamento".into()),
                dur,
                format::hms(dur),
                origin
            ));
        }
        Ok(out)
    }
}

// ====================================================================== auxiliares SQL

fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn date_to_db(d: NaiveDate) -> i32 {
    d.num_days_from_ce()
}

fn date_from_db(v: i32) -> NaiveDate {
    NaiveDate::from_num_days_from_ce_opt(v).unwrap_or_default()
}

fn get_kv(conn: &Connection, k: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| r.get(0)).optional()?)
}

fn set_kv(conn: &Connection, k: &str, v: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [k, v],
    )?;
    Ok(())
}

fn del_kv(conn: &Connection, k: &str) -> Result<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [k])?;
    Ok(())
}

fn set_bool(conn: &Connection, k: &str, v: bool) -> Result<()> {
    set_kv(conn, k, if v { "1" } else { "0" })
}

fn get_bool(conn: &Connection, k: &str, default: bool) -> Result<bool> {
    Ok(get_kv(conn, k)?.map_or(default, |v| v == "1"))
}

fn read_settings(conn: &Connection) -> Result<Settings> {
    let d = Settings::default();
    let bed = get_kv(conn, key::BEDTIME)?.and_then(|v| v.parse::<ClockTime>().ok());
    let wake = get_kv(conn, key::WAKE)?.and_then(|v| v.parse::<ClockTime>().ok());
    let schedule = match (bed, wake) {
        (Some(b), Some(w)) => SleepSchedule::new(b, w).unwrap_or(d.schedule),
        _ => d.schedule,
    };
    let retention = get_kv(conn, key::RETENTION)?.and_then(|v| v.parse::<u32>().ok()).filter(|&v| v > 0);
    Ok(Settings {
        schedule,
        overtime: get_bool(conn, key::OVERTIME, d.overtime)?,
        sound: get_bool(conn, key::SOUND, d.sound)?,
        vibration: get_bool(conn, key::VIBRATION, d.vibration)?,
        retention_days: retention,
        onboarded: get_bool(conn, key::ONBOARDED, false)?,
    })
}

fn write_schedule(conn: &Connection, s: SleepSchedule) -> Result<()> {
    set_kv(conn, key::BEDTIME, &s.bedtime().to_string())?;
    set_kv(conn, key::WAKE, &s.wake().to_string())
}

fn focus_tag(conn: &Connection) -> Result<Option<i64>> {
    Ok(get_kv(conn, key::FOCUS)?.and_then(|v| v.parse().ok()))
}

fn tag_from_row(r: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: r.get(0)?,
        name: r.get(1)?,
        color: r.get(2)?,
        permille: Permille::saturating(r.get(3)?),
        position: r.get(4)?,
        archived: r.get(5)?,
    })
}

fn get_tag(conn: &Connection, id: i64) -> Result<Tag> {
    conn.query_row("SELECT id, name, color, permille, position, archived FROM tags WHERE id = ?1", [id], tag_from_row)
        .optional()?
        .ok_or(Error::TagNotFound)
}

fn active_count(conn: &Connection) -> Result<usize> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM tags WHERE archived = 0", [], |r| r.get(0))?;
    Ok(n as usize)
}

fn active_total(conn: &Connection, except: Option<i64>) -> Result<Permille> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(SUM(permille), 0) FROM tags WHERE archived = 0 AND id != ?1",
        [except.unwrap_or(-1)],
        |r| r.get(0),
    )?;
    Ok(Permille::saturating(v))
}

fn next_position(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM tags", [], |r| r.get(0))?)
}

fn ensure_unique_name(conn: &Connection, key: &str, name: &str, except: Option<i64>) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE archived = 0 AND name_key = ?1 AND id != ?2)",
        params![key, except.unwrap_or(-1)],
        |r| r.get(0),
    )?;
    if exists {
        return Err(CoreError::TagNameDuplicate(name.to_string()).into());
    }
    Ok(())
}

fn insert_tag(conn: &Connection, new: &NewTag, now: i64) -> Result<i64> {
    let name = normalize_name(&new.name)?;
    let key = name_key(&name);
    let p = Permille::new(new.permille.value())?;
    if active_count(conn)? >= MAX_ACTIVE_TAGS {
        return Err(CoreError::TooManyTags.into());
    }
    ensure_unique_name(conn, &key, &name, None)?;
    check_allocation(active_total(conn, None)?, p)?;
    conn.execute(
        "INSERT INTO tags (name, name_key, color, permille, position, archived, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        params![name, key, new.color & 0xFF_FF_FF, p.value(), next_position(conn)?, now],
    )?;
    Ok(conn.last_insert_rowid())
}

fn day_from_row(r: &Row<'_>) -> rusqlite::Result<DayRow> {
    Ok(DayRow { id: r.get(0)?, date: date_from_db(r.get(1)?), start: r.get(2)?, end: r.get(3)?, available: r.get(4)? })
}

fn open_day(conn: &Connection) -> Result<Option<DayRow>> {
    Ok(conn
        .query_row(
            "SELECT id, date, start_utc, end_utc, available_secs FROM days WHERE closed = 0
             ORDER BY start_utc DESC LIMIT 1",
            [],
            day_from_row,
        )
        .optional()?)
}

fn insert_day(tx: &Transaction<'_>, date: NaiveDate, start: i64, end: i64, available: i64) -> Result<DayRow> {
    tx.execute(
        "INSERT INTO days (date, start_utc, end_utc, available_secs, closed, no_record) VALUES (?1, ?2, ?3, ?4, 0, 0)",
        params![date_to_db(date), start, end, available],
    )?;
    let day = DayRow { id: tx.last_insert_rowid(), date, start, end, available };
    snapshot_allocations(tx, &day)?;
    Ok(day)
}

/// RN28: copia as alocações vigentes para o dia.
fn snapshot_allocations(tx: &Transaction<'_>, day: &DayRow) -> Result<()> {
    let mut st = tx.prepare("SELECT id, permille FROM tags WHERE archived = 0")?;
    let tags =
        st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, p) in tags {
        let p = Permille::saturating(p);
        tx.execute(
            "INSERT OR REPLACE INTO day_allocations (day_id, tag_id, permille, allocated_secs) VALUES (?1, ?2, ?3, ?4)",
            params![day.id, id, p.value(), allocated_secs(day.available, p)],
        )?;
    }
    Ok(())
}

fn ensure_allocation_row(conn: &Connection, day: &DayRow, tag: &Tag) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO day_allocations (day_id, tag_id, permille, allocated_secs) VALUES (?1, ?2, ?3, ?4)",
        params![day.id, tag.id, tag.permille.value(), allocated_secs(day.available, tag.permille)],
    )?;
    Ok(())
}

/// RN15: atualiza a alocação da tag no dia atual.
fn sync_today_allocation(conn: &Connection, tag_id: i64) -> Result<()> {
    let Some(day) = open_day(conn)? else { return Ok(()) };
    let tag = get_tag(conn, tag_id)?;
    conn.execute(
        "INSERT OR REPLACE INTO day_allocations (day_id, tag_id, permille, allocated_secs) VALUES (?1, ?2, ?3, ?4)",
        params![day.id, tag.id, tag.permille.value(), allocated_secs(day.available, tag.permille)],
    )?;
    Ok(())
}

fn create_day_containing(tx: &Transaction<'_>, clock: &dyn Clock, now: i64, s: &Settings) -> Result<DayRow> {
    let b = day_bounds_containing(clock, now, s.schedule.wake());
    // A data precisa ser posterior a qualquer dia já registrado.
    let last: Option<i32> = tx.query_row("SELECT MAX(date) FROM days", [], |r| r.get(0))?;
    let date = match last {
        Some(l) if l >= date_to_db(b.date) => date_from_db(l + 1),
        _ => b.date,
    };
    insert_day(tx, date, b.start, b.end, s.schedule.available_secs())
}

fn close_day(tx: &Transaction<'_>, id: i64) -> Result<()> {
    tx.execute(
        "UPDATE days SET closed = 1,
                no_record = NOT EXISTS (SELECT 1 FROM sessions WHERE day_id = ?1)
         WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

/// Fecha o dia e abre o seguinte, que começa exatamente onde o anterior terminou.
/// Um timer ativo é dividido na fronteira (RN21). O novo dia usa o sono vigente (RN16).
fn roll_over(tx: &Transaction<'_>, clock: &dyn Clock, day: &DayRow, s: &Settings) -> Result<DayRow> {
    let active = active_session(tx)?;
    if let Some(a) = active {
        close_session(tx, a.id, day.end)?;
    }
    close_day(tx, day.id)?;

    let date = day.date.succ_opt().unwrap_or(day.date);
    let bounds = day_bounds_for_date(clock, date, s.schedule.wake());
    let start = day.end;
    let end = bounds.end.max(start + 3_600);
    let next = insert_day(tx, date, start, end, s.schedule.available_secs())?;

    if let Some(a) = active {
        tx.execute(
            "INSERT INTO sessions (day_id, tag_id, start_utc, end_utc, origin) VALUES (?1, ?2, ?3, NULL, 'timer')",
            params![next.id, a.tag_id, start],
        )?;
        let tag = get_tag(tx, a.tag_id)?;
        ensure_allocation_row(tx, &next, &tag)?;
    }
    Ok(next)
}

fn apply_retention(conn: &Connection, retention: Option<u32>, today: NaiveDate) -> Result<()> {
    if let Some(n) = retention {
        conn.execute("DELETE FROM days WHERE closed = 1 AND date < ?1", [date_to_db(today) - n as i32])?;
    }
    Ok(())
}

fn active_session(conn: &Connection) -> Result<Option<ActiveRow>> {
    Ok(conn
        .query_row(
            "SELECT id, day_id, tag_id, start_utc FROM sessions WHERE end_utc IS NULL ORDER BY start_utc DESC LIMIT 1",
            [],
            |r| Ok(ActiveRow { id: r.get(0)?, day_id: r.get(1)?, tag_id: r.get(2)?, start: r.get(3)? }),
        )
        .optional()?)
}

fn close_session(conn: &Connection, id: i64, end: i64) -> Result<()> {
    conn.execute("UPDATE sessions SET end_utc = MAX(?1, start_utc) WHERE id = ?2", params![end, id])?;
    Ok(())
}

fn day_spans(conn: &Connection, day_id: i64) -> Result<Vec<tempo_core::Span>> {
    let mut st = conn.prepare("SELECT start_utc, end_utc FROM sessions WHERE day_id = ?1")?;
    let rows = st.query_map([day_id], |r| Ok(tempo_core::Span { start: r.get(0)?, end: r.get(1)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn consumed_by_tag(conn: &Connection, day_id: i64, now: i64) -> Result<Vec<(i64, i64)>> {
    let mut st = conn.prepare(
        "SELECT tag_id, SUM(MAX(COALESCE(end_utc, ?2) - start_utc, 0)) FROM sessions WHERE day_id = ?1 GROUP BY tag_id",
    )?;
    let rows = st.query_map([day_id, now], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn allocation(conn: &Connection, day: &DayRow, tag_id: i64) -> Result<i64> {
    Ok(conn
        .query_row(
            "SELECT allocated_secs FROM day_allocations WHERE day_id = ?1 AND tag_id = ?2",
            [day.id, tag_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn closed_consumed(conn: &Connection, day_id: i64, tag_id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(MAX(end_utc - start_utc, 0)), 0) FROM sessions
         WHERE day_id = ?1 AND tag_id = ?2 AND end_utc IS NOT NULL",
        [day_id, tag_id],
        |r| r.get(0),
    )?)
}

fn tag_progress(conn: &Connection, day: &DayRow, tag_id: i64, now: i64) -> Result<Progress> {
    let consumed = consumed_by_tag(conn, day.id, now)?.into_iter().find(|(id, _)| *id == tag_id).map_or(0, |(_, c)| c);
    Ok(Progress::new(allocation(conn, day, tag_id)?, consumed))
}

fn active_timer(conn: &Connection, day: &DayRow) -> Result<Option<ActiveTimer>> {
    let Some(a) = active_session(conn)? else { return Ok(None) };
    let tag = get_tag(conn, a.tag_id)?;
    let before = closed_consumed(conn, a.day_id, a.tag_id)?;
    let allocated = allocation(conn, day, a.tag_id)?;
    Ok(Some(ActiveTimer {
        session_id: a.id,
        tag_id: tag.id,
        tag_name: tag.name,
        color: tag.color,
        start: a.start,
        consumed_before: before,
        allocated,
        deadline: deadline(a.start, before, allocated),
    }))
}

/// RN20: sem excedente, encerra a sessão no instante exato em que o alocado foi atingido.
fn auto_stop(conn: &Connection, day: &DayRow, limit: i64) -> Result<Option<i64>> {
    let Some(a) = active_session(conn)? else { return Ok(None) };
    let before = closed_consumed(conn, a.day_id, a.tag_id)?;
    let dl = deadline(a.start, before, allocation(conn, day, a.tag_id)?);
    if dl <= limit {
        close_session(conn, a.id, dl)?;
        return Ok(Some(a.tag_id));
    }
    Ok(None)
}

/// RN24: se o relógio voltou, encerra a sessão ativa no último instante conhecido.
fn check_clock(conn: &Connection, now: i64, report: &mut TickReport) -> Result<()> {
    let last: Option<i64> = get_kv(conn, key::LAST_SEEN)?.and_then(|v| v.parse().ok());
    if let Some(last) = last {
        if now + CLOCK_BACK_TOLERANCE < last {
            if let Some(a) = active_session(conn)? {
                close_session(conn, a.id, last)?;
                set_kv(conn, key::CLOCK_WARNING, "1")?;
                report.clock_went_back = true;
            }
            set_kv(conn, key::LAST_SEEN, &now.to_string())?;
            return Ok(());
        }
    }
    if last.is_none_or(|l| now - l >= LAST_SEEN_WRITE_EVERY) {
        set_kv(conn, key::LAST_SEEN, &now.to_string())?;
    }
    Ok(())
}

fn day_tag_summaries(conn: &Connection, day_id: i64, now: i64) -> Result<Vec<DayTagSummary>> {
    let consumed = consumed_by_tag(conn, day_id, now)?;
    let mut st = conn.prepare(
        "SELECT t.id, t.name, t.color, a.permille, a.allocated_secs
         FROM day_allocations a JOIN tags t ON t.id = a.tag_id
         WHERE a.day_id = ?1 ORDER BY t.archived, t.position",
    )?;
    let rows = st.query_map([day_id], |r| {
        Ok(DayTagSummary {
            tag_id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            permille: Permille::saturating(r.get(3)?),
            allocated: r.get(4)?,
            consumed: 0,
        })
    })?;
    let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for t in &mut out {
        t.consumed = consumed.iter().find(|(id, _)| *id == t.tag_id).map_or(0, |(_, c)| *c);
    }
    Ok(out)
}
