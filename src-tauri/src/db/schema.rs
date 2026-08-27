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
    // 0002 — Watch State replaces Resume Points (ADR-0006), and every Viewer
    // row carries a name snapshot so a Sync can spot a renumbered id (ADR-0007).
    r#"
    CREATE TABLE watch_state (
        provider_id   INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        kind          TEXT    NOT NULL CHECK (kind IN ('movie','episode')),
        ref_id        TEXT    NOT NULL,
        state         TEXT    NOT NULL CHECK (state IN ('in_progress','watched')),
        position_secs INTEGER,
        duration_secs INTEGER,
        name_snapshot TEXT,
        updated_at    INTEGER NOT NULL,
        PRIMARY KEY (provider_id, kind, ref_id),
        -- A position is exactly what makes a row In Progress. Watched rows have
        -- none: there is nothing left to return to.
        CHECK ((state = 'in_progress') = (position_secs IS NOT NULL))
    ) WITHOUT ROWID;
    CREATE INDEX watch_state_recent ON watch_state(provider_id, updated_at DESC);

    INSERT INTO watch_state
        (provider_id, kind, ref_id, state, position_secs, duration_secs, updated_at)
    SELECT provider_id, kind, ref_id, 'in_progress', position_secs, duration_secs, updated_at
    FROM resume_points;

    DROP TABLE resume_points;

    -- Rebuilt rather than altered: SQLite cannot widen a CHECK in place, and a
    -- Series is now favouritable even though it is not a Playable.
    CREATE TABLE favourites_v2 (
        provider_id   INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
        kind          TEXT    NOT NULL CHECK (kind IN ('channel','movie','episode','series')),
        ref_id        TEXT    NOT NULL,
        name_snapshot TEXT,
        created_at    INTEGER NOT NULL,
        PRIMARY KEY (provider_id, kind, ref_id)
    ) WITHOUT ROWID;

    INSERT INTO favourites_v2 (provider_id, kind, ref_id, created_at)
    SELECT provider_id, kind, ref_id, created_at FROM favourites;

    DROP TABLE favourites;
    ALTER TABLE favourites_v2 RENAME TO favourites;

    -- Backfill snapshots for rows that predate them, so the first Sync after
    -- this migration can already tell a renumber from a removal.
    UPDATE favourites SET name_snapshot = (
        SELECT c.name FROM channels c
         WHERE c.provider_id = favourites.provider_id
           AND CAST(c.stream_id AS TEXT) = favourites.ref_id
    ) WHERE kind = 'channel';
    UPDATE favourites SET name_snapshot = (
        SELECT m.name FROM movies m
         WHERE m.provider_id = favourites.provider_id
           AND CAST(m.stream_id AS TEXT) = favourites.ref_id
    ) WHERE kind = 'movie';
    UPDATE favourites SET name_snapshot = (
        SELECT e.title FROM episodes e
         WHERE e.provider_id = favourites.provider_id
           AND e.episode_id = favourites.ref_id
    ) WHERE kind = 'episode';
    UPDATE watch_state SET name_snapshot = (
        SELECT m.name FROM movies m
         WHERE m.provider_id = watch_state.provider_id
           AND CAST(m.stream_id AS TEXT) = watch_state.ref_id
    ) WHERE kind = 'movie';
    UPDATE watch_state SET name_snapshot = (
        SELECT e.title FROM episodes e
         WHERE e.provider_id = watch_state.provider_id
           AND e.episode_id = watch_state.ref_id
    ) WHERE kind = 'episode';
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
        // One transaction per migration. 0002 rebuilds `favourites`, and a
        // half-applied rebuild would leave no table at all.
        conn.execute_batch("BEGIN")?;
        match conn.execute_batch(sql) {
            Ok(()) => {
                conn.pragma_update(None, "user_version", (i + 1) as i64)?;
                conn.execute_batch("COMMIT")?;
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e.into());
            }
        }
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

    /// Applies only the first N migrations, to stand in for an older database.
    fn migrate_to(conn: &Connection, upto: usize) {
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        for (i, sql) in MIGRATIONS.iter().enumerate().take(upto) {
            conn.execute_batch(sql).unwrap();
            conn.pragma_update(None, "user_version", (i + 1) as i64)
                .unwrap();
        }
    }

    /// The upgrade path a real installation takes, not just a fresh install.
    #[test]
    fn migration_0002_carries_existing_viewer_data_across() {
        let conn = Connection::open_in_memory().unwrap();
        migrate_to(&conn, 1);

        conn.execute_batch(
            "INSERT INTO providers (id, name, base_url, username, created_at)
                  VALUES (1, 'P', 'http://x', 'u', 0);
             INSERT INTO channels (provider_id, stream_id, name, has_archive)
                  VALUES (1, 42, 'LA 1', 0);
             INSERT INTO movies (provider_id, stream_id, name)
                  VALUES (1, 7, 'Amelie');
             INSERT INTO favourites VALUES (1, 'channel', '42', 0);
             INSERT INTO resume_points VALUES (1, 'movie', '7', 300, 6000, 0);",
        )
        .unwrap();

        migrate(&conn).unwrap();

        // The Favourite survived and picked up a snapshot to reconcile against.
        let (kind, snapshot): (String, Option<String>) = conn
            .query_row(
                "SELECT kind, name_snapshot FROM favourites WHERE ref_id = '42'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "channel");
        assert_eq!(snapshot.as_deref(), Some("LA 1"));

        // The Resume Point became an In Progress Watch State, position intact.
        let (state, pos, snap): (String, i64, Option<String>) = conn
            .query_row(
                "SELECT state, position_secs, name_snapshot FROM watch_state WHERE ref_id = '7'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, "in_progress");
        assert_eq!(pos, 300);
        assert_eq!(snap.as_deref(), Some("Amelie"));

        // And a Series is now favouritable, which the old CHECK forbade.
        conn.execute(
            "INSERT INTO favourites VALUES (1,'series','100',NULL,0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn a_watched_row_cannot_carry_a_position() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO providers (id,name,base_url,username,created_at) VALUES (1,'P','u','u',0)",
            [],
        )
        .unwrap();
        assert!(
            conn.execute(
                "INSERT INTO watch_state VALUES (1,'movie','7','watched',300,NULL,NULL,0)",
                [],
            )
            .is_err(),
            "a Watched row has nothing to return to, so it must have no position"
        );
    }

    #[test]
    fn a_channel_cannot_have_a_resume_point() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let err = conn.execute(
            "INSERT INTO watch_state VALUES (1,'channel','5','in_progress',10,NULL,NULL,0)",
            [],
        );
        assert!(err.is_err(), "the CHECK constraint should reject 'channel'");
    }
}
