use rusqlite::Connection;

use crate::error::Result;

/// Ordered, append-only. Never edit a migration that has shipped; add another.
const MIGRATIONS: &[&str] = &[
    // 0001 — Providers, Catalogue, viewing state.
    r#"
    CREATE TABLE providers (
        id             INTEGER PRIMARY KEY,
        name           TEXT    NOT NULL UNIQUE,
        base_url       TEXT    NOT NULL,
        username       TEXT    NOT NULL,
        created_at     INTEGER NOT NULL,
        last_synced_at INTEGER,
        status         TEXT,
        expires_at     INTEGER,
        max_sessions   INTEGER
    );

    -- Every Catalogue row is scoped to a Provider from the first migration.
    -- Retrofitting this later would mean migrating the whole mirror (ADR-0005).
    CREATE TABLE categories (
        provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        kind        TEXT    NOT NULL CHECK (kind IN ('live','movie','series')),
        category_id INTEGER NOT NULL,
        name        TEXT    NOT NULL,
        PRIMARY KEY (provider_id, kind, category_id)
    ) WITHOUT ROWID;

    CREATE TABLE channels (
        provider_id     INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        stream_id       INTEGER NOT NULL,
        name            TEXT    NOT NULL,
        icon            TEXT,
        epg_channel_id  TEXT,
        category_id     INTEGER,
        channel_number  INTEGER,
        has_archive     INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (provider_id, stream_id)
    ) WITHOUT ROWID;
    CREATE INDEX channels_by_category ON channels(provider_id, category_id, channel_number);

    CREATE TABLE movies (
        provider_id         INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        stream_id           INTEGER NOT NULL,
        name                TEXT    NOT NULL,
        icon                TEXT,
        category_id         INTEGER,
        container_extension TEXT,
        rating              REAL,
        added_at            INTEGER,
        PRIMARY KEY (provider_id, stream_id)
    ) WITHOUT ROWID;
    CREATE INDEX movies_by_category ON movies(provider_id, category_id, name);
    CREATE INDEX movies_by_added ON movies(provider_id, added_at DESC);

    CREATE TABLE series (
        provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        series_id   INTEGER NOT NULL,
        name        TEXT    NOT NULL,
        cover       TEXT,
        plot        TEXT,
        category_id INTEGER,
        rating      REAL,
        PRIMARY KEY (provider_id, series_id)
    ) WITHOUT ROWID;
    CREATE INDEX series_by_category ON series(provider_id, category_id, name);

    -- Episodes are fetched lazily per Series, not during a full Sync: pulling
    -- get_series_info for 5000 Series would be 5000 round trips.
    CREATE TABLE episodes (
        provider_id         INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        episode_id          TEXT    NOT NULL,
        series_id           INTEGER NOT NULL,
        season              INTEGER NOT NULL,
        episode_number      INTEGER NOT NULL,
        title               TEXT    NOT NULL,
        plot                TEXT,
        container_extension TEXT,
        duration_secs       INTEGER,
        PRIMARY KEY (provider_id, episode_id)
    ) WITHOUT ROWID;
    CREATE INDEX episodes_by_series ON episodes(provider_id, series_id, season, episode_number);

    CREATE TABLE programmes (
        provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        stream_id   INTEGER NOT NULL,
        start_ts    INTEGER NOT NULL,
        stop_ts     INTEGER NOT NULL,
        title       TEXT    NOT NULL,
        description TEXT,
        PRIMARY KEY (provider_id, stream_id, start_ts)
    ) WITHOUT ROWID;

    CREATE TABLE favourites (
        provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        kind        TEXT    NOT NULL CHECK (kind IN ('channel','movie','episode')),
        ref_id      TEXT    NOT NULL,
        created_at  INTEGER NOT NULL,
        PRIMARY KEY (provider_id, kind, ref_id)
    ) WITHOUT ROWID;

    -- Channels are excluded by the CHECK: live has no beginning to return to.
    CREATE TABLE resume_points (
        provider_id   INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        kind          TEXT    NOT NULL CHECK (kind IN ('movie','episode')),
        ref_id        TEXT    NOT NULL,
        position_secs INTEGER NOT NULL,
        duration_secs INTEGER,
        updated_at    INTEGER NOT NULL,
        PRIMARY KEY (provider_id, kind, ref_id)
    ) WITHOUT ROWID;

    -- One search index across all three kinds, so a single query answers
    -- "where is Formula 1" without three round trips.
    CREATE VIRTUAL TABLE playables_fts USING fts5(
        name,
        provider_id UNINDEXED,
        kind        UNINDEXED,
        ref_id      UNINDEXED,
        tokenize    = "unicode61 remove_diacritics 2"
    );
    "#,
];

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // The Catalogue is a rebuildable mirror, so durability matters less than
    // the speed of writing 80k rows.
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    let applied: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(applied as usize) {
        tracing::info!(migration = i + 1, "applying migration");
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", (i + 1) as i64)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_to_a_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
    }

    #[test]
    fn a_channel_cannot_have_a_resume_point() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let err = conn.execute(
            "INSERT INTO resume_points VALUES (1,'channel','5',10,NULL,0)",
            [],
        );
        assert!(err.is_err(), "the CHECK constraint should reject 'channel'");
    }
}
