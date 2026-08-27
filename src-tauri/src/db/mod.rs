pub mod model;
pub mod schema;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, Result};
use crate::xtream::{CatalogueKind, PlayableKind};
use model::*;

/// The local mirror of every **Provider**'s **Catalogue** (ADR-0004).
///
/// One connection behind a mutex. The write path is a handful of bulk syncs;
/// the read path is fast enough that contention never shows up in a UI.
pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        schema::migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned mutex means a previous query panicked. The mirror is
        // rebuildable, so carrying on beats taking the app down.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- Providers -------------------------------------------------------

    pub fn add_provider(&self, name: &str, base_url: &str, username: &str) -> Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO providers (name, base_url, username, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, base_url, username, now()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn remove_provider(&self, id: i64) -> Result<()> {
        let conn = self.conn();
        // ON DELETE CASCADE clears the Catalogue; the FTS index is not a real
        // foreign key, so it has to go by hand.
        conn.execute("DELETE FROM playables_fts WHERE provider_id = ?1", [id])?;
        let n = conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(AppError::UnknownProvider(id));
        }
        Ok(())
    }

    pub fn provider_credentials(&self, id: i64) -> Result<(String, String)> {
        self.conn()
            .query_row(
                "SELECT base_url, username FROM providers WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(AppError::UnknownProvider(id))
    }

    pub fn providers(&self) -> Result<Vec<Provider>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.base_url, p.username, p.last_synced_at,
                    p.status, p.expires_at, p.max_sessions,
                    (SELECT COUNT(*) FROM channels c WHERE c.provider_id = p.id),
                    (SELECT COUNT(*) FROM movies   m WHERE m.provider_id = p.id),
                    (SELECT COUNT(*) FROM series   s WHERE s.provider_id = p.id)
             FROM providers p
             ORDER BY p.created_at",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Provider {
                id: r.get(0)?,
                name: r.get(1)?,
                base_url: r.get(2)?,
                username: r.get(3)?,
                last_synced_at: r.get(4)?,
                entitlement: Entitlement {
                    status: r.get(5)?,
                    expires_at: r.get(6)?,
                    max_sessions: r.get(7)?,
                },
                counts: CatalogueCounts {
                    channels: r.get(8)?,
                    movies: r.get(9)?,
                    series: r.get(10)?,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn record_entitlement(
        &self,
        provider_id: i64,
        status: Option<&str>,
        expires_at: Option<i64>,
        max_sessions: Option<i64>,
    ) -> Result<()> {
        self.conn().execute(
            "UPDATE providers SET status = ?2, expires_at = ?3, max_sessions = ?4
             WHERE id = ?1",
            params![provider_id, status, expires_at, max_sessions],
        )?;
        Ok(())
    }

    pub fn mark_synced(&self, provider_id: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE providers SET last_synced_at = ?2 WHERE id = ?1",
            params![provider_id, now()],
        )?;
        Ok(())
    }

    // ---- Catalogue reads -------------------------------------------------

    pub fn categories(&self, provider_id: i64, kind: CatalogueKind) -> Result<Vec<Category>> {
        let table = match kind {
            CatalogueKind::Live => "channels",
            CatalogueKind::Movie => "movies",
            CatalogueKind::Series => "series",
        };
        let conn = self.conn();
        let sql = format!(
            "SELECT c.category_id, c.name,
                    (SELECT COUNT(*) FROM {table} t
                      WHERE t.provider_id = c.provider_id AND t.category_id = c.category_id)
             FROM categories c
             WHERE c.provider_id = ?1 AND c.kind = ?2
             ORDER BY c.name"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![provider_id, kind.as_str()], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn channels(
        &self,
        provider_id: i64,
        category_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Channel>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT c.stream_id, c.name, c.icon, c.category_id, c.channel_number, c.has_archive,
                    c.epg_channel_id,
                    EXISTS (SELECT 1 FROM favourites f
                             WHERE f.provider_id = c.provider_id
                               AND f.kind = 'channel'
                               AND f.ref_id = CAST(c.stream_id AS TEXT))
             FROM channels c
             WHERE c.provider_id = ?1 AND (?2 IS NULL OR c.category_id = ?2)
             ORDER BY c.channel_number, c.name
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(params![provider_id, category_id, limit, offset], |r| {
            Ok(Channel {
                stream_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                category_id: r.get(3)?,
                channel_number: r.get(4)?,
                has_archive: r.get::<_, i64>(5)? != 0,
                epg_channel_id: r.get(6)?,
                is_favourite: r.get::<_, i64>(7)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn movies(
        &self,
        provider_id: i64,
        category_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Movie>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT m.stream_id, m.name, m.icon, m.category_id, m.container_extension, m.rating,
                    m.added_at,
                    EXISTS (SELECT 1 FROM favourites f
                             WHERE f.provider_id = m.provider_id AND f.kind = 'movie'
                               AND f.ref_id = CAST(m.stream_id AS TEXT)),
                    r.position_secs, r.duration_secs
             FROM movies m
             LEFT JOIN resume_points r
                    ON r.provider_id = m.provider_id AND r.kind = 'movie'
                   AND r.ref_id = CAST(m.stream_id AS TEXT)
             WHERE m.provider_id = ?1 AND (?2 IS NULL OR m.category_id = ?2)
             ORDER BY m.name
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(params![provider_id, category_id, limit, offset], |r| {
            Ok(Movie {
                stream_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                category_id: r.get(3)?,
                container_extension: r.get(4)?,
                rating: r.get(5)?,
                added_at: r.get(6)?,
                is_favourite: r.get::<_, i64>(7)? != 0,
                resume: r
                    .get::<_, Option<i64>>(8)?
                    .map(|position_secs| ResumePoint {
                        position_secs,
                        duration_secs: r.get(9).ok().flatten(),
                    }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn series(
        &self,
        provider_id: i64,
        category_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Series>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT series_id, name, cover, plot, category_id, rating
             FROM series
             WHERE provider_id = ?1 AND (?2 IS NULL OR category_id = ?2)
             ORDER BY name
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(params![provider_id, category_id, limit, offset], |r| {
            Ok(Series {
                series_id: r.get(0)?,
                name: r.get(1)?,
                cover: r.get(2)?,
                plot: r.get(3)?,
                category_id: r.get(4)?,
                rating: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn episodes(&self, provider_id: i64, series_id: i64) -> Result<Vec<Episode>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT e.episode_id, e.series_id, e.season, e.episode_number, e.title, e.plot,
                    e.container_extension, e.duration_secs, r.position_secs, r.duration_secs
             FROM episodes e
             LEFT JOIN resume_points r
                    ON r.provider_id = e.provider_id AND r.kind = 'episode'
                   AND r.ref_id = e.episode_id
             WHERE e.provider_id = ?1 AND e.series_id = ?2
             ORDER BY e.season, e.episode_number",
        )?;
        let rows = stmt.query_map(params![provider_id, series_id], |r| {
            Ok(Episode {
                episode_id: r.get(0)?,
                series_id: r.get(1)?,
                season: r.get(2)?,
                episode_number: r.get(3)?,
                title: r.get(4)?,
                plot: r.get(5)?,
                container_extension: r.get(6)?,
                duration_secs: r.get(7)?,
                resume: r
                    .get::<_, Option<i64>>(8)?
                    .map(|position_secs| ResumePoint {
                        position_secs,
                        duration_secs: r.get(9).ok().flatten(),
                    }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn search(&self, provider_id: i64, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let cleaned = fts_query(query);
        if cleaned.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT kind, ref_id, name
             FROM playables_fts
             WHERE playables_fts MATCH ?2 AND provider_id = ?1
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![provider_id, cleaned, limit], |r| {
            Ok(SearchHit {
                kind: r.get(0)?,
                ref_id: r.get(1)?,
                name: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn schedule(&self, provider_id: i64, stream_id: i64, from: i64) -> Result<Vec<Programme>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT start_ts, stop_ts, title, description
             FROM programmes
             WHERE provider_id = ?1 AND stream_id = ?2 AND stop_ts >= ?3
             ORDER BY start_ts",
        )?;
        let rows = stmt.query_map(params![provider_id, stream_id, from], |r| {
            Ok(Programme {
                start_ts: r.get(0)?,
                stop_ts: r.get(1)?,
                title: r.get(2)?,
                description: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn replace_schedule(
        &self,
        provider_id: i64,
        stream_id: i64,
        programmes: &[Programme],
    ) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM programmes WHERE provider_id = ?1 AND stream_id = ?2",
            params![provider_id, stream_id],
        )?;
        {
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO programmes
                 (provider_id, stream_id, start_ts, stop_ts, title, description)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for p in programmes {
                ins.execute(params![
                    provider_id,
                    stream_id,
                    p.start_ts,
                    p.stop_ts,
                    p.title,
                    p.description
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- Viewing state ---------------------------------------------------

    pub fn toggle_favourite(&self, r: &PlayableRef) -> Result<bool> {
        let conn = self.conn();
        let removed = conn.execute(
            "DELETE FROM favourites WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3",
            params![r.provider_id, r.kind.as_str(), r.ref_id],
        )?;
        if removed > 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO favourites (provider_id, kind, ref_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![r.provider_id, r.kind.as_str(), r.ref_id, now()],
        )?;
        Ok(true)
    }

    pub fn save_resume_point(
        &self,
        r: &PlayableRef,
        position: i64,
        duration: Option<i64>,
    ) -> Result<()> {
        // A Channel has no beginning to return to (CONTEXT.md). The schema
        // would reject it anyway; failing quietly here keeps the caller simple.
        if !r.kind.resumable() {
            return Ok(());
        }
        // Finished, or barely started: neither is worth resuming.
        let finished = duration.is_some_and(|d| d > 0 && position as f64 / d as f64 > 0.95);
        if position < 30 || finished {
            self.conn().execute(
                "DELETE FROM resume_points WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3",
                params![r.provider_id, r.kind.as_str(), r.ref_id],
            )?;
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO resume_points (provider_id, kind, ref_id, position_secs, duration_secs, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (provider_id, kind, ref_id) DO UPDATE SET
                position_secs = excluded.position_secs,
                duration_secs = excluded.duration_secs,
                updated_at    = excluded.updated_at",
            params![r.provider_id, r.kind.as_str(), r.ref_id, position, duration, now()],
        )?;
        Ok(())
    }

    pub fn resume_point(&self, r: &PlayableRef) -> Result<Option<ResumePoint>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT position_secs, duration_secs FROM resume_points
                 WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3",
                params![r.provider_id, r.kind.as_str(), r.ref_id],
                |row| {
                    Ok(ResumePoint {
                        position_secs: row.get(0)?,
                        duration_secs: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    /// Looks up the container extension a **Playable** needs in its
    /// **Stream URL**. Live has none; VOD guesses `mp4` when the panel omits it.
    pub fn container_extension(&self, r: &PlayableRef) -> Result<Option<String>> {
        let conn = self.conn();
        let sql = match r.kind {
            PlayableKind::Channel => return Ok(None),
            PlayableKind::Movie => {
                "SELECT container_extension FROM movies
                 WHERE provider_id = ?1 AND CAST(stream_id AS TEXT) = ?2"
            }
            PlayableKind::Episode => {
                "SELECT container_extension FROM episodes
                 WHERE provider_id = ?1 AND episode_id = ?2"
            }
        };
        Ok(conn
            .query_row(sql, params![r.provider_id, r.ref_id], |row| row.get(0))
            .optional()?
            .flatten())
    }

    /// Human-readable label for a **Playable**, used as the mpv window title.
    pub fn playable_title(&self, r: &PlayableRef) -> Result<Option<String>> {
        let conn = self.conn();
        let sql = match r.kind {
            PlayableKind::Channel => {
                "SELECT name FROM channels WHERE provider_id = ?1 AND CAST(stream_id AS TEXT) = ?2"
            }
            PlayableKind::Movie => {
                "SELECT name FROM movies WHERE provider_id = ?1 AND CAST(stream_id AS TEXT) = ?2"
            }
            PlayableKind::Episode => {
                "SELECT title FROM episodes WHERE provider_id = ?1 AND episode_id = ?2"
            }
        };
        Ok(conn
            .query_row(sql, params![r.provider_id, r.ref_id], |row| row.get(0))
            .optional()?)
    }

    // ---- Bulk writes -----------------------------------------------------

    /// Replaces one kind of the **Catalogue** wholesale, inside one transaction.
    ///
    /// Wholesale, not incremental: providers renumber ids between panel
    /// upgrades, so diffing would leave orphans that never disappear.
    pub fn replace_catalogue(&self, provider_id: i64, batch: CatalogueBatch) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM categories WHERE provider_id = ?1",
            [provider_id],
        )?;
        tx.execute("DELETE FROM channels WHERE provider_id = ?1", [provider_id])?;
        tx.execute("DELETE FROM movies WHERE provider_id = ?1", [provider_id])?;
        tx.execute("DELETE FROM series WHERE provider_id = ?1", [provider_id])?;
        tx.execute(
            "DELETE FROM playables_fts WHERE provider_id = ?1",
            [provider_id],
        )?;

        {
            let mut cat = tx.prepare(
                "INSERT OR REPLACE INTO categories (provider_id, kind, category_id, name)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (kind, id, name) in &batch.categories {
                cat.execute(params![provider_id, kind.as_str(), id, name])?;
            }

            let mut fts = tx.prepare(
                "INSERT INTO playables_fts (name, provider_id, kind, ref_id)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;

            let mut ch = tx.prepare(
                "INSERT OR REPLACE INTO channels
                 (provider_id, stream_id, name, icon, epg_channel_id, category_id,
                  channel_number, has_archive)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for c in &batch.channels {
                ch.execute(params![
                    provider_id,
                    c.stream_id,
                    c.name,
                    c.icon,
                    c.epg_channel_id,
                    c.category_id,
                    c.channel_number,
                    c.has_archive as i64
                ])?;
                fts.execute(params![
                    c.name,
                    provider_id,
                    "channel",
                    c.stream_id.to_string()
                ])?;
            }

            let mut mv = tx.prepare(
                "INSERT OR REPLACE INTO movies
                 (provider_id, stream_id, name, icon, category_id, container_extension,
                  rating, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for m in &batch.movies {
                mv.execute(params![
                    provider_id,
                    m.stream_id,
                    m.name,
                    m.icon,
                    m.category_id,
                    m.container_extension,
                    m.rating,
                    m.added_at
                ])?;
                fts.execute(params![
                    m.name,
                    provider_id,
                    "movie",
                    m.stream_id.to_string()
                ])?;
            }

            let mut sr = tx.prepare(
                "INSERT OR REPLACE INTO series
                 (provider_id, series_id, name, cover, plot, category_id, rating)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for s in &batch.series {
                sr.execute(params![
                    provider_id,
                    s.series_id,
                    s.name,
                    s.cover,
                    s.plot,
                    s.category_id,
                    s.rating
                ])?;
                // Series are not playable, but they are searchable — the hit
                // navigates to the Series page rather than starting playback.
                fts.execute(params![
                    s.name,
                    provider_id,
                    "series",
                    s.series_id.to_string()
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn replace_episodes(
        &self,
        provider_id: i64,
        series_id: i64,
        episodes: &[Episode],
    ) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM episodes WHERE provider_id = ?1 AND series_id = ?2",
            params![provider_id, series_id],
        )?;
        {
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO episodes
                 (provider_id, episode_id, series_id, season, episode_number, title, plot,
                  container_extension, duration_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for e in episodes {
                ins.execute(params![
                    provider_id,
                    e.episode_id,
                    series_id,
                    e.season,
                    e.episode_number,
                    e.title,
                    e.plot,
                    e.container_extension,
                    e.duration_secs
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// One **Sync**'s worth of **Catalogue**, ready to be written in one go.
#[derive(Default)]
pub struct CatalogueBatch {
    pub categories: Vec<(CatalogueKind, i64, String)>,
    pub channels: Vec<Channel>,
    pub movies: Vec<Movie>,
    pub series: Vec<Series>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Turns free text into a safe FTS5 prefix query.
///
/// FTS5 treats `"`, `*`, `:`, `^` and `-` as syntax, and channel names are full
/// of them (`SPAIN | LA 1 -HD-`). Quoting each word and appending `*` gives
/// prefix matching without ever handing user text to the parser raw.
fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| w.replace(['"', '*', ':', '^'], " "))
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> (Db, i64) {
        let db = Db::open_in_memory().unwrap();
        let id = db
            .add_provider("Test", "http://example.com", "bob")
            .unwrap();
        db.replace_catalogue(
            id,
            CatalogueBatch {
                categories: vec![(CatalogueKind::Live, 1, "Spain".into())],
                channels: vec![Channel {
                    stream_id: 42,
                    name: "SPAIN | LA 1 -HD-".into(),
                    icon: None,
                    category_id: Some(1),
                    channel_number: Some(1),
                    has_archive: false,
                    epg_channel_id: None,
                    is_favourite: false,
                }],
                movies: vec![Movie {
                    stream_id: 7,
                    name: "Amélie".into(),
                    icon: None,
                    category_id: Some(1),
                    container_extension: Some("mkv".into()),
                    rating: Some(8.3),
                    added_at: None,
                    is_favourite: false,
                    resume: None,
                }],
                series: vec![],
            },
        )
        .unwrap();
        (db, id)
    }

    #[test]
    fn search_survives_punctuation_in_the_query() {
        let (db, id) = seeded();
        // Raw, this would be an FTS5 syntax error.
        let hits = db.search(id, "LA 1 -HD-", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "channel");
    }

    #[test]
    fn search_matches_on_a_prefix() {
        let (db, id) = seeded();
        assert_eq!(db.search(id, "spa", 10).unwrap().len(), 1);
    }

    #[test]
    fn search_ignores_diacritics() {
        let (db, id) = seeded();
        assert_eq!(db.search(id, "amelie", 10).unwrap().len(), 1);
    }

    #[test]
    fn search_is_scoped_to_one_provider() {
        let (db, id) = seeded();
        let other = db.add_provider("Other", "http://b.com", "ann").unwrap();
        assert_eq!(db.search(other, "spa", 10).unwrap().len(), 0);
        assert_eq!(db.search(id, "spa", 10).unwrap().len(), 1);
    }

    #[test]
    fn removing_a_provider_clears_its_catalogue_and_search_index() {
        let (db, id) = seeded();
        db.remove_provider(id).unwrap();
        assert_eq!(db.channels(id, None, 100, 0).unwrap().len(), 0);
        assert_eq!(db.search(id, "spa", 10).unwrap().len(), 0);
    }

    #[test]
    fn a_channel_never_gets_a_resume_point() {
        let (db, id) = seeded();
        let r = PlayableRef {
            provider_id: id,
            kind: PlayableKind::Channel,
            ref_id: "42".into(),
        };
        db.save_resume_point(&r, 600, None).unwrap();
        assert!(db.resume_point(&r).unwrap().is_none());
    }

    #[test]
    fn a_nearly_finished_movie_drops_its_resume_point() {
        let (db, id) = seeded();
        let r = PlayableRef {
            provider_id: id,
            kind: PlayableKind::Movie,
            ref_id: "7".into(),
        };
        db.save_resume_point(&r, 300, Some(6000)).unwrap();
        assert!(db.resume_point(&r).unwrap().is_some());

        db.save_resume_point(&r, 5900, Some(6000)).unwrap();
        assert!(db.resume_point(&r).unwrap().is_none(), "95% is finished");
    }

    #[test]
    fn the_first_thirty_seconds_are_not_worth_resuming() {
        let (db, id) = seeded();
        let r = PlayableRef {
            provider_id: id,
            kind: PlayableKind::Movie,
            ref_id: "7".into(),
        };
        db.save_resume_point(&r, 12, Some(6000)).unwrap();
        assert!(db.resume_point(&r).unwrap().is_none());
    }

    #[test]
    fn favourites_toggle_both_ways() {
        let (db, id) = seeded();
        let r = PlayableRef {
            provider_id: id,
            kind: PlayableKind::Channel,
            ref_id: "42".into(),
        };
        assert!(db.toggle_favourite(&r).unwrap());
        assert!(db.channels(id, None, 10, 0).unwrap()[0].is_favourite);
        assert!(!db.toggle_favourite(&r).unwrap());
        assert!(!db.channels(id, None, 10, 0).unwrap()[0].is_favourite);
    }

    #[test]
    fn categories_carry_their_item_count() {
        let (db, id) = seeded();
        let cats = db.categories(id, CatalogueKind::Live).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].count, 1);
    }
}
