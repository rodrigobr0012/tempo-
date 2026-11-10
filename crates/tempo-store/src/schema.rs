//! Esquema do banco e migrações versionadas por `PRAGMA user_version` (RNF25).

use rusqlite::Connection;

/// Cada item é a migração para a versão `índice + 1`. Nunca altere uma migração já publicada:
/// acrescente uma nova no fim da lista.
const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE tags (
        id         INTEGER PRIMARY KEY,
        name       TEXT    NOT NULL,
        name_key   TEXT    NOT NULL,
        color      INTEGER NOT NULL,
        permille   INTEGER NOT NULL CHECK (permille BETWEEN 0 AND 1000),
        position   INTEGER NOT NULL,
        archived   INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    -- RN11: nome único entre as tags ativas
    CREATE UNIQUE INDEX tags_active_name ON tags(name_key) WHERE archived = 0;

    CREATE TABLE days (
        id             INTEGER PRIMARY KEY,
        date           INTEGER NOT NULL UNIQUE, -- dias desde a era comum (data local de início)
        start_utc      INTEGER NOT NULL,
        end_utc        INTEGER NOT NULL,
        available_secs INTEGER NOT NULL,        -- RN28: snapshot do TD
        closed         INTEGER NOT NULL DEFAULT 0,
        no_record      INTEGER NOT NULL DEFAULT 0
    );

    -- RN28: snapshot das alocações de cada dia
    CREATE TABLE day_allocations (
        day_id         INTEGER NOT NULL REFERENCES days(id) ON DELETE CASCADE,
        tag_id         INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        permille       INTEGER NOT NULL,
        allocated_secs INTEGER NOT NULL,
        PRIMARY KEY (day_id, tag_id)
    );

    CREATE TABLE sessions (
        id        INTEGER PRIMARY KEY,
        day_id    INTEGER NOT NULL REFERENCES days(id) ON DELETE CASCADE,
        tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        start_utc INTEGER NOT NULL,
        end_utc   INTEGER,                      -- NULL = timer rodando
        origin    TEXT    NOT NULL CHECK (origin IN ('timer', 'manual'))
    );
    CREATE INDEX sessions_day ON sessions(day_id, tag_id);
    CREATE INDEX sessions_active ON sessions(end_utc) WHERE end_utc IS NULL;
    "#,
    // v2: tempo reservado definido pelo usuário (substitui o par dormir/acordar)
    r#"
    CREATE TABLE reserved (
        id            INTEGER PRIMARY KEY,
        name          TEXT    NOT NULL,
        name_key      TEXT    NOT NULL UNIQUE,
        duration_secs INTEGER NOT NULL CHECK (duration_secs > 0),
        position      INTEGER NOT NULL,
        created_at    INTEGER NOT NULL
    );
    "#,
];

#[cfg(test)]
pub const CURRENT_VERSION: i64 = MIGRATIONS.len() as i64;

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version.max(0) as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migracao_e_idempotente() {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        migrate(&mut c).unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, CURRENT_VERSION);
    }
}
