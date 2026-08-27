pub mod model;
pub mod schema;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, Result};
use crate::region;
use crate::xtream::{decode_category_ref, CatalogueKind, FavouriteKind, PlayableKind};
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
        let db = Self(Mutex::new(conn));
        db.backfill_regions()?;
        db.rebuild_search_index_if_empty()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    /// Tags **Categories** that predate Regions, so an upgraded database works
    /// immediately instead of only after the next **Sync**.
    ///
    /// A NULL `region_code` means "never computed"; `OTHER` means "computed and
    /// nothing matched" — so this never re-runs over rows it has already seen.
    fn backfill_regions(&self) -> Result<()> {
        let pending: Vec<(String, i64, String)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare(
                "SELECT kind, category_id, name FROM categories WHERE region_code IS NULL",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        if pending.is_empty() {
            return Ok(());
        }
        tracing::info!(count = pending.len(), "tagging categories with a region");

        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut upd = tx.prepare(
                "UPDATE categories SET region_code = ?3 WHERE kind = ?1 AND category_id = ?2",
            )?;
            for (kind, id, name) in &pending {
                let code = region::region_for_category(name).unwrap_or(region::OTHER);
                upd.execute(params![kind, id, code])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Rebuilds the search index from the mirror when it is empty but the
    /// **Catalogue** is not.
    ///
    /// Migration 0004 drops the index to add a column. Without this, search
    /// would come back empty after an upgrade and stay that way until the next
    /// **Sync** — a silent failure, and the worst kind.
    fn rebuild_search_index_if_empty(&self) -> Result<()> {
        let conn = self.conn();
        let indexed: i64 =
            conn.query_row("SELECT COUNT(*) FROM playables_fts", [], |r| r.get(0))?;
        let channels: i64 = conn.query_row("SELECT COUNT(*) FROM channels", [], |r| r.get(0))?;
        if indexed > 0 || channels == 0 {
            return Ok(());
        }
        drop(conn);

        tracing::info!("rebuilding the search index after a schema change");
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for (table, kind, id_col, cat_kind) in [
            ("channels", "channel", "stream_id", "live"),
            ("movies", "movie", "stream_id", "movie"),
            ("series", "series", "series_id", "series"),
        ] {
            tx.execute(
                &format!(
                    "INSERT INTO playables_fts (name, provider_id, kind, ref_id, region_code)
                     SELECT t.name, t.provider_id, ?1, CAST(t.{id_col} AS TEXT),
                            COALESCE(
                                (SELECT c.region_code FROM categories c
                                  WHERE c.provider_id = t.provider_id
                                    AND c.kind = ?2
                                    AND c.category_id = t.category_id),
                                ?3)
                       FROM {table} t"
                ),
                params![kind, cat_kind, region::OTHER],
            )?;
        }
        tx.commit()?;
        Ok(())
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

    /// **Categories** for one part of the **Catalogue**, filtered to the
    /// **Regions** the **Viewer** shows and ordered Region, then **Favourite**,
    /// then name.
    ///
    /// Ordered in Rust rather than SQL because the Region's position comes from
    /// `region_settings` and its tie-break is a human label, neither of which
    /// SQL can reach without a join that obscures the rule.
    pub fn categories(&self, provider_id: i64, kind: CatalogueKind) -> Result<Vec<Category>> {
        let settings = self.region_settings(provider_id)?;
        let curated = self.regions_curated(provider_id)?;

        let table = match kind {
            CatalogueKind::Live => "channels",
            CatalogueKind::Movie => "movies",
            CatalogueKind::Series => "series",
        };
        let conn = self.conn();
        let sql = format!(
            "SELECT c.category_id, c.name, c.region_code,
                    (SELECT COUNT(*) FROM {table} t
                      WHERE t.provider_id = c.provider_id AND t.category_id = c.category_id),
                    EXISTS (SELECT 1 FROM favourites f
                             WHERE f.provider_id = c.provider_id AND f.kind = 'category'
                               AND f.ref_id = ?3 || ':' || c.category_id)
             FROM categories c
             WHERE c.provider_id = ?1 AND c.kind = ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![provider_id, kind.as_str(), kind.as_str()], |r| {
            let code: Option<String> = r.get(2)?;
            let code = code.unwrap_or_else(|| region::OTHER.to_string());
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                region_label: region::region_label(&code).to_string(),
                region_code: code,
                count: r.get(3)?,
                is_favourite: r.get::<_, i64>(4)? != 0,
            })
        })?;
        let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        out.retain(|c| is_region_visible(&settings, &c.region_code, curated));
        sort_categories(&mut out, &settings);
        Ok(out)
    }

    /// Every **Region** present in this **Provider**'s **Catalogue**, for Settings.
    pub fn regions(&self, provider_id: i64) -> Result<Vec<Region>> {
        let settings = self.region_settings(provider_id)?;
        let curated = self.regions_curated(provider_id)?;

        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT COALESCE(region_code, ?2), COUNT(*)
             FROM categories WHERE provider_id = ?1
             GROUP BY COALESCE(region_code, ?2)",
        )?;
        let rows = stmt.query_map(params![provider_id, region::OTHER], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let counts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut out: Vec<Region> = counts
            .into_iter()
            .map(|(code, category_count)| {
                let setting = settings.get(&code);
                Region {
                    label: region::region_label(&code).to_string(),
                    visible: is_region_visible(&settings, &code, curated),
                    sort_order: setting.map(|s| s.1).unwrap_or(i64::MAX),
                    // Curated, but no row: this Region arrived in a later Sync
                    // and was hidden on arrival.
                    is_new: curated && setting.is_none(),
                    code,
                    category_count,
                }
            })
            .collect();

        out.sort_by(|a, b| {
            a.sort_order
                .cmp(&b.sort_order)
                .then_with(|| other_last(&a.code).cmp(&other_last(&b.code)))
                .then_with(|| a.label.cmp(&b.label))
        });
        Ok(out)
    }

    fn region_settings(&self, provider_id: i64) -> Result<HashMap<String, (bool, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT region_code, visible, sort_order FROM region_settings WHERE provider_id = ?1",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)? != 0, r.get::<_, i64>(2)?),
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
    }

    /// A **Provider** that no longer exists has curated nothing. Callers reach
    /// here while tearing one down, and that is not an error worth raising.
    fn regions_curated(&self, provider_id: i64) -> Result<bool> {
        Ok(self
            .conn()
            .query_row(
                "SELECT regions_curated FROM providers WHERE id = ?1",
                [provider_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some_and(|v| v != 0))
    }

    /// Shows or hides a **Region**.
    ///
    /// The first call materialises a row for every Region currently known, so
    /// that from then on "no row" unambiguously means "arrived in a later
    /// Sync" — which is what makes hiding new Regions safe (ADR-0008).
    pub fn set_region_visible(&self, provider_id: i64, code: &str, visible: bool) -> Result<()> {
        self.ensure_curated(provider_id)?;
        self.conn().execute(
            "UPDATE region_settings SET visible = ?3
              WHERE provider_id = ?1 AND region_code = ?2",
            params![provider_id, code, visible as i64],
        )?;
        Ok(())
    }

    /// Reorders the shown **Regions**; `codes` is the order the Viewer wants.
    ///
    /// Anything not named falls to the end. Without that reset, a Region the
    /// Viewer never ordered keeps `sort_order = 0` and outranks one they
    /// deliberately placed second.
    pub fn set_region_order(&self, provider_id: i64, codes: &[String]) -> Result<()> {
        self.ensure_curated(provider_id)?;
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE region_settings SET sort_order = ?2 WHERE provider_id = ?1",
            params![provider_id, i64::from(u32::MAX)],
        )?;
        for (i, code) in codes.iter().enumerate() {
            tx.execute(
                "UPDATE region_settings SET sort_order = ?3
                  WHERE provider_id = ?1 AND region_code = ?2",
                params![provider_id, code, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn ensure_curated(&self, provider_id: i64) -> Result<()> {
        if self.regions_curated(provider_id)? {
            return Ok(());
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO region_settings (provider_id, region_code, visible, sort_order)
             SELECT ?1, COALESCE(region_code, ?2), 1, 0
               FROM categories WHERE provider_id = ?1
              GROUP BY COALESCE(region_code, ?2)",
            params![provider_id, region::OTHER],
        )?;
        tx.execute(
            "UPDATE providers SET regions_curated = 1 WHERE id = ?1",
            [provider_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The **Viewer** correcting what the heuristic got wrong. Survives a Sync.
    pub fn set_category_region(
        &self,
        provider_id: i64,
        kind: CatalogueKind,
        category_id: i64,
        code: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO category_region_override
                (provider_id, catalogue_kind, category_id, region_code)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (provider_id, catalogue_kind, category_id)
             DO UPDATE SET region_code = excluded.region_code",
            params![provider_id, kind.as_str(), category_id, code],
        )?;
        conn.execute(
            "UPDATE categories SET region_code = ?4
              WHERE provider_id = ?1 AND kind = ?2 AND category_id = ?3",
            params![provider_id, kind.as_str(), category_id, code],
        )?;
        Ok(())
    }

    /// **Regions** that appeared in the last **Sync** and were hidden on
    /// arrival, so the Sync panel can say so rather than losing them silently.
    pub fn new_region_count(&self, provider_id: i64) -> Result<i64> {
        if !self.regions_curated(provider_id)? {
            return Ok(0);
        }
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM (
                 SELECT COALESCE(region_code, ?2) AS code
                   FROM categories WHERE provider_id = ?1
                  GROUP BY code
             ) c
             WHERE NOT EXISTS (
                 SELECT 1 FROM region_settings rs
                  WHERE rs.provider_id = ?1 AND rs.region_code = c.code
             )",
            params![provider_id, region::OTHER],
            |r| r.get(0),
        )?)
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
             LEFT JOIN watch_state r
                    ON r.provider_id = m.provider_id AND r.kind = 'movie'
                   AND r.ref_id = CAST(m.stream_id AS TEXT)
                   AND r.state = 'in_progress'
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
             LEFT JOIN watch_state r
                    ON r.provider_id = e.provider_id AND r.kind = 'episode'
                   AND r.ref_id = e.episode_id
                   AND r.state = 'in_progress'
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
        let curated = self.regions_curated(provider_id)?;
        let conn = self.conn();
        // Hidden Regions are hidden here too, not just in the rail: search is
        // where 15k Channels hurt most.
        let mut stmt = conn.prepare(
            "SELECT kind, ref_id, name
             FROM playables_fts
             WHERE playables_fts MATCH ?2 AND provider_id = ?1
               AND (?4 = 0 OR EXISTS (
                     SELECT 1 FROM region_settings rs
                      WHERE rs.provider_id = ?1
                        AND rs.region_code = playables_fts.region_code
                        AND rs.visible = 1
                   ))
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![provider_id, cleaned, limit, curated as i64], |r| {
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

    pub fn toggle_favourite(&self, r: &FavouriteRef) -> Result<bool> {
        let conn = self.conn();
        let removed = conn.execute(
            "DELETE FROM favourites WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3",
            params![r.provider_id, r.kind.as_str(), r.ref_id],
        )?;
        if removed > 0 {
            return Ok(false);
        }
        // The snapshot is what lets a later Sync tell a renumbered id from a
        // removed one (ADR-0007).
        let name = self.name_of(&conn, r)?;
        conn.execute(
            "INSERT INTO favourites (provider_id, kind, ref_id, name_snapshot, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![r.provider_id, r.kind.as_str(), r.ref_id, name, now()],
        )?;
        Ok(true)
    }

    pub fn favourites(&self, provider_id: i64) -> Result<Favourites> {
        Ok(Favourites {
            channels: self.favourite_channels(provider_id)?,
            movies: self.favourite_movies(provider_id)?,
            series: self.favourite_series(provider_id)?,
        })
    }

    fn favourite_channels(&self, provider_id: i64) -> Result<Vec<Channel>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT c.stream_id, c.name, c.icon, c.category_id, c.channel_number,
                    c.has_archive, c.epg_channel_id
             FROM favourites f
             JOIN channels c
               ON c.provider_id = f.provider_id AND CAST(c.stream_id AS TEXT) = f.ref_id
             WHERE f.provider_id = ?1 AND f.kind = 'channel'
             ORDER BY f.created_at",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
            Ok(Channel {
                stream_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                category_id: r.get(3)?,
                channel_number: r.get(4)?,
                has_archive: r.get::<_, i64>(5)? != 0,
                epg_channel_id: r.get(6)?,
                is_favourite: true,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn favourite_movies(&self, provider_id: i64) -> Result<Vec<Movie>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT m.stream_id, m.name, m.icon, m.category_id, m.container_extension,
                    m.rating, m.added_at, w.position_secs, w.duration_secs
             FROM favourites f
             JOIN movies m
               ON m.provider_id = f.provider_id AND CAST(m.stream_id AS TEXT) = f.ref_id
             LEFT JOIN watch_state w
               ON w.provider_id = f.provider_id AND w.kind = 'movie'
              AND w.ref_id = f.ref_id AND w.state = 'in_progress'
             WHERE f.provider_id = ?1 AND f.kind = 'movie'
             ORDER BY f.created_at",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
            Ok(Movie {
                stream_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                category_id: r.get(3)?,
                container_extension: r.get(4)?,
                rating: r.get(5)?,
                added_at: r.get(6)?,
                is_favourite: true,
                resume: r
                    .get::<_, Option<i64>>(7)?
                    .map(|position_secs| ResumePoint {
                        position_secs,
                        duration_secs: r.get(8).ok().flatten(),
                    }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn favourite_series(&self, provider_id: i64) -> Result<Vec<Series>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT s.series_id, s.name, s.cover, s.plot, s.category_id, s.rating
             FROM favourites f
             JOIN series s
               ON s.provider_id = f.provider_id AND CAST(s.series_id AS TEXT) = f.ref_id
             WHERE f.provider_id = ?1 AND f.kind = 'series'
             ORDER BY f.created_at",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
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

    pub fn is_favourite(&self, r: &FavouriteRef) -> Result<bool> {
        Ok(self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM favourites
                             WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3)",
            params![r.provider_id, r.kind.as_str(), r.ref_id],
            |row| row.get::<_, i64>(0),
        )? != 0)
    }

    /// Records progress, promoting to **Watched** at the end (ADR-0006).
    ///
    /// Completion is stored, not deleted — deleting it is what made **Up Next**
    /// uncomputable.
    pub fn save_watch_state(
        &self,
        r: &PlayableRef,
        position: i64,
        duration: Option<i64>,
    ) -> Result<()> {
        // A Channel has no beginning to return to (CONTEXT.md), and the CHECK
        // constraint would reject it anyway.
        if !r.kind.resumable() {
            return Ok(());
        }

        let finished = duration.is_some_and(|d| d > 0 && position as f64 / d as f64 > 0.95);
        if finished {
            return self.set_watch_state(r, WatchState::Watched, None, duration);
        }
        // Under half a minute is not progress, it is a misclick. Leave whatever
        // state the row already had rather than overwriting it.
        if position < 30 {
            return Ok(());
        }
        self.set_watch_state(r, WatchState::InProgress, Some(position), duration)
    }

    /// The **Viewer** saying "I meant to skip that" — the escape hatch for the
    /// one case Up Next gets wrong (ADR-0006).
    pub fn mark_watched(&self, r: &PlayableRef) -> Result<()> {
        if !r.kind.resumable() {
            return Ok(());
        }
        self.set_watch_state(r, WatchState::Watched, None, None)
    }

    pub fn clear_watch_state(&self, r: &PlayableRef) -> Result<()> {
        self.conn().execute(
            "DELETE FROM watch_state WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3",
            params![r.provider_id, r.kind.as_str(), r.ref_id],
        )?;
        Ok(())
    }

    fn set_watch_state(
        &self,
        r: &PlayableRef,
        state: WatchState,
        position: Option<i64>,
        duration: Option<i64>,
    ) -> Result<()> {
        let name = self.name_of(&self.conn(), &FavouriteRef::from(r))?;
        self.conn().execute(
            "INSERT INTO watch_state
                (provider_id, kind, ref_id, state, position_secs, duration_secs,
                 name_snapshot, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (provider_id, kind, ref_id) DO UPDATE SET
                state         = excluded.state,
                position_secs = excluded.position_secs,
                duration_secs = COALESCE(excluded.duration_secs, watch_state.duration_secs),
                name_snapshot = COALESCE(excluded.name_snapshot, watch_state.name_snapshot),
                updated_at    = excluded.updated_at",
            params![
                r.provider_id,
                r.kind.as_str(),
                r.ref_id,
                state.as_str(),
                position,
                duration,
                name,
                now()
            ],
        )?;
        Ok(())
    }

    /// Where to start playing, if the **Viewer** is part-way through.
    pub fn resume_point(&self, r: &PlayableRef) -> Result<Option<ResumePoint>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT position_secs, duration_secs FROM watch_state
                  WHERE provider_id = ?1 AND kind = ?2 AND ref_id = ?3
                    AND state = 'in_progress'",
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

    /// **Continue Watching**: everything **In Progress**, plus the **Up Next**
    /// of every started **Series**.
    ///
    /// At most one row per Series. If an **Episode** of it is In Progress that
    /// wins; otherwise Up Next stands in. Showing both would put the same show
    /// on Home twice.
    pub fn continue_watching(&self, provider_id: i64, limit: i64) -> Result<Vec<ContinueItem>> {
        let mut items = self.in_progress_movies(provider_id)?;
        let in_progress_episodes = self.in_progress_episodes(provider_id)?;

        let covered: std::collections::HashSet<i64> = in_progress_episodes
            .iter()
            .filter_map(|i| i.series_id)
            .collect();
        items.extend(in_progress_episodes);

        for (series_id, updated_at) in self.started_series(provider_id)? {
            if covered.contains(&series_id) {
                continue;
            }
            if let Some(mut up_next) = self.up_next(provider_id, series_id)? {
                // Order Home by when the Series was last touched, not by the
                // Episode's own (nonexistent) timestamp.
                up_next.updated_at = updated_at;
                items.push(up_next);
            }
        }

        items.sort_by_key(|i| std::cmp::Reverse(i.updated_at));
        items.truncate(limit as usize);
        Ok(items)
    }

    fn in_progress_movies(&self, provider_id: i64) -> Result<Vec<ContinueItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT w.ref_id, m.name, m.icon, w.position_secs, w.duration_secs, w.updated_at
             FROM watch_state w
             JOIN movies m
               ON m.provider_id = w.provider_id AND CAST(m.stream_id AS TEXT) = w.ref_id
             WHERE w.provider_id = ?1 AND w.kind = 'movie' AND w.state = 'in_progress'",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
            Ok(ContinueItem {
                kind: PlayableKind::Movie,
                ref_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                position_secs: r.get(3)?,
                duration_secs: r.get(4)?,
                updated_at: r.get(5)?,
                is_up_next: false,
                series_id: None,
                series_name: None,
                season: None,
                episode_number: None,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn in_progress_episodes(&self, provider_id: i64) -> Result<Vec<ContinueItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT w.ref_id, e.title, s.cover, w.position_secs, w.duration_secs, w.updated_at,
                    e.series_id, s.name, e.season, e.episode_number
             FROM watch_state w
             JOIN episodes e
               ON e.provider_id = w.provider_id AND e.episode_id = w.ref_id
             LEFT JOIN series s
               ON s.provider_id = e.provider_id AND s.series_id = e.series_id
             WHERE w.provider_id = ?1 AND w.kind = 'episode' AND w.state = 'in_progress'",
        )?;
        let rows = stmt.query_map([provider_id], |r| {
            Ok(ContinueItem {
                kind: PlayableKind::Episode,
                ref_id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                position_secs: r.get(3)?,
                duration_secs: r.get(4)?,
                updated_at: r.get(5)?,
                is_up_next: false,
                series_id: r.get(6)?,
                series_name: r.get(7)?,
                season: r.get(8)?,
                episode_number: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// **Series** the **Viewer** has touched, with when they last touched them.
    fn started_series(&self, provider_id: i64) -> Result<Vec<(i64, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT e.series_id, MAX(w.updated_at)
             FROM watch_state w
             JOIN episodes e
               ON e.provider_id = w.provider_id AND e.episode_id = w.ref_id
             WHERE w.provider_id = ?1 AND w.kind = 'episode'
             GROUP BY e.series_id",
        )?;
        let rows = stmt.query_map([provider_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The lowest **Unwatched** **Episode** of a **Series** (ADR-0006).
    ///
    /// Unwatched means *no row at all* — an **In Progress** Episode is already
    /// surfaced in its own right, and offering it twice would be noise. Note
    /// this is ordered by season and episode number, never by recency: doing it
    /// by recency sends the Viewer backwards after a rewatch.
    pub fn up_next(&self, provider_id: i64, series_id: i64) -> Result<Option<ContinueItem>> {
        let conn = self.conn();
        Ok(conn
            .query_row(
                "SELECT e.episode_id, e.title, s.cover, e.season, e.episode_number, s.name
                 FROM episodes e
                 LEFT JOIN series s
                   ON s.provider_id = e.provider_id AND s.series_id = e.series_id
                 WHERE e.provider_id = ?1 AND e.series_id = ?2
                   AND NOT EXISTS (
                       SELECT 1 FROM watch_state w
                        WHERE w.provider_id = e.provider_id AND w.kind = 'episode'
                          AND w.ref_id = e.episode_id
                   )
                 ORDER BY e.season, e.episode_number
                 LIMIT 1",
                params![provider_id, series_id],
                |r| {
                    Ok(ContinueItem {
                        kind: PlayableKind::Episode,
                        ref_id: r.get(0)?,
                        name: r.get(1)?,
                        icon: r.get(2)?,
                        position_secs: None,
                        duration_secs: None,
                        updated_at: 0,
                        is_up_next: true,
                        series_id: Some(series_id),
                        series_name: r.get(5)?,
                        season: r.get(3)?,
                        episode_number: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// The current name of whatever a ref points at, for the snapshot.
    fn name_of(&self, conn: &Connection, r: &FavouriteRef) -> Result<Option<String>> {
        // A Category ref carries a pair packed into one string, so it cannot
        // share the single-id lookup the others use.
        if r.kind == FavouriteKind::Category {
            let Some((kind, id)) = decode_category_ref(&r.ref_id) else {
                return Ok(None);
            };
            return Ok(conn
                .query_row(
                    "SELECT name FROM categories
                      WHERE provider_id = ?1 AND kind = ?2 AND category_id = ?3",
                    params![r.provider_id, kind.as_str(), id],
                    |row| row.get(0),
                )
                .optional()?);
        }

        let sql = match r.kind {
            FavouriteKind::Channel => {
                "SELECT name FROM channels WHERE provider_id = ?1 AND CAST(stream_id AS TEXT) = ?2"
            }
            FavouriteKind::Movie => {
                "SELECT name FROM movies WHERE provider_id = ?1 AND CAST(stream_id AS TEXT) = ?2"
            }
            FavouriteKind::Episode => {
                "SELECT title FROM episodes WHERE provider_id = ?1 AND episode_id = ?2"
            }
            FavouriteKind::Series => {
                "SELECT name FROM series WHERE provider_id = ?1 AND CAST(series_id AS TEXT) = ?2"
            }
            FavouriteKind::Category => unreachable!("handled above"),
        };
        Ok(conn
            .query_row(sql, params![r.provider_id, r.ref_id], |row| row.get(0))
            .optional()?
            .flatten())
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

        let mut region_by_category: HashMap<(&str, i64), String> = HashMap::new();
        {
            let mut cat = tx.prepare(
                "INSERT OR REPLACE INTO categories
                    (provider_id, kind, category_id, name, region_code)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            let mut override_lookup = tx.prepare(
                "SELECT region_code FROM category_region_override
                  WHERE provider_id = ?1 AND catalogue_kind = ?2 AND category_id = ?3",
            )?;
            for (kind, id, name) in &batch.categories {
                // A Viewer's correction outranks the heuristic and outlives the
                // wipe above, which is why it lives in its own table.
                let overridden: Option<Option<String>> = override_lookup
                    .query_row(params![provider_id, kind.as_str(), id], |r| r.get(0))
                    .optional()?;
                let region = match overridden {
                    Some(Some(code)) => code,
                    // An override explicitly cleared still means Other.
                    Some(None) => region::OTHER.to_string(),
                    None => region::region_for_category(name)
                        .unwrap_or(region::OTHER)
                        .to_string(),
                };
                cat.execute(params![provider_id, kind.as_str(), id, name, &region])?;
                region_by_category.insert((kind.as_str(), *id), region);
            }

            let mut fts = tx.prepare(
                "INSERT INTO playables_fts (name, provider_id, kind, ref_id, region_code)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            // Denormalised onto the index so a hidden Region drops out of
            // search with one lookup instead of a join per hit.
            let region_of = |kind: &str, category_id: Option<i64>| -> String {
                category_id
                    .and_then(|id| region_by_category.get(&(kind, id)).cloned())
                    .unwrap_or_else(|| region::OTHER.to_string())
            };

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
                    c.stream_id.to_string(),
                    region_of("live", c.category_id)
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
                    m.stream_id.to_string(),
                    region_of("movie", m.category_id)
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
                    s.series_id.to_string(),
                    region_of("series", s.category_id)
                ])?;
            }
        }

        reconcile_refs(
            &tx,
            provider_id,
            &["channel", "movie", "series", "category"],
        )?;

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

        // Episode-kind rows are reconciled here rather than in
        // `replace_catalogue`, because a full Sync never fetches Episodes — the
        // `episodes` table is empty for every Series the Viewer has not opened,
        // so reconciling there would wipe the Watch State of every show they
        // are part-way through.
        reconcile_refs(&tx, provider_id, &["episode"])?;

        tx.commit()?;
        Ok(())
    }
}

/// Drops **Favourites** and **Watch State** a **Sync** has invalidated (ADR-0007).
///
/// A row dies if its `ref_id` no longer exists, or if the name it points at has
/// changed — a **Provider** that renumbers ids does not remove id 42, it
/// reassigns it, so a missing-id check alone would leave the Favourite pointing
/// at a different **Channel** with nothing to show for it.
///
/// Survivors then have their snapshot refreshed, so a later legitimate retitle
/// is measured against what was actually there last Sync.
fn reconcile_refs(
    tx: &rusqlite::Transaction<'_>,
    provider_id: i64,
    kinds: &[&str],
) -> Result<usize> {
    let mut dropped = 0;

    for kind in kinds {
        if *kind == "category" {
            // The ref packs catalogue-kind and id together, so the join is on
            // the encoded string rather than a bare column.
            dropped += tx.execute(
                "DELETE FROM favourites
                  WHERE provider_id = ?1 AND kind = 'category'
                    AND NOT EXISTS (
                        SELECT 1 FROM categories c
                         WHERE c.provider_id = favourites.provider_id
                           AND c.kind || ':' || c.category_id = favourites.ref_id
                           AND (favourites.name_snapshot IS NULL
                                OR c.name = favourites.name_snapshot)
                    )",
                [provider_id],
            )?;
            tx.execute(
                "UPDATE favourites SET name_snapshot = (
                     SELECT c.name FROM categories c
                      WHERE c.provider_id = favourites.provider_id
                        AND c.kind || ':' || c.category_id = favourites.ref_id
                 )
                  WHERE provider_id = ?1 AND kind = 'category'",
                [provider_id],
            )?;
            continue;
        }

        let (table, id_expr, name_col) = match *kind {
            "channel" => ("channels", "CAST(t.stream_id AS TEXT)", "t.name"),
            "movie" => ("movies", "CAST(t.stream_id AS TEXT)", "t.name"),
            "series" => ("series", "CAST(t.series_id AS TEXT)", "t.name"),
            "episode" => ("episodes", "t.episode_id", "t.title"),
            other => return Err(AppError::Other(format!("unknown ref kind {other}"))),
        };

        for viewer_table in ["favourites", "watch_state"] {
            // watch_state only ever holds Movies and Episodes.
            if viewer_table == "watch_state" && !matches!(*kind, "movie" | "episode") {
                continue;
            }
            dropped += tx.execute(
                &format!(
                    "DELETE FROM {viewer_table}
                      WHERE provider_id = ?1 AND kind = ?2
                        AND NOT EXISTS (
                            SELECT 1 FROM {table} t
                             WHERE t.provider_id = {viewer_table}.provider_id
                               AND {id_expr} = {viewer_table}.ref_id
                               AND ({viewer_table}.name_snapshot IS NULL
                                    OR {name_col} = {viewer_table}.name_snapshot)
                        )"
                ),
                params![provider_id, kind],
            )?;

            tx.execute(
                &format!(
                    "UPDATE {viewer_table} SET name_snapshot = (
                         SELECT {name_col} FROM {table} t
                          WHERE t.provider_id = {viewer_table}.provider_id
                            AND {id_expr} = {viewer_table}.ref_id
                     )
                      WHERE provider_id = ?1 AND kind = ?2"
                ),
                params![provider_id, kind],
            )?;
        }
    }

    if dropped > 0 {
        tracing::info!(dropped, provider_id, "dropped stale viewer refs after sync");
    }
    Ok(dropped)
}

/// One **Sync**'s worth of **Catalogue**, ready to be written in one go.
#[derive(Default)]
pub struct CatalogueBatch {
    pub categories: Vec<(CatalogueKind, i64, String)>,
    pub channels: Vec<Channel>,
    pub movies: Vec<Movie>,
    pub series: Vec<Series>,
}

/// Before the **Viewer** curates, everything shows. After, only Regions they
/// kept — a missing row means the Region arrived later (ADR-0008).
fn is_region_visible(settings: &HashMap<String, (bool, i64)>, code: &str, curated: bool) -> bool {
    match settings.get(code) {
        Some((visible, _)) => *visible,
        None => !curated,
    }
}

/// `Other` sorts last whatever else happens; it is the leftovers bucket.
fn other_last(code: &str) -> u8 {
    u8::from(code == region::OTHER)
}

fn sort_categories(items: &mut [Category], settings: &HashMap<String, (bool, i64)>) {
    items.sort_by(|a, b| {
        let order = |c: &Category| {
            settings
                .get(&c.region_code)
                .map(|s| s.1)
                .unwrap_or(i64::MAX)
        };
        order(a)
            .cmp(&order(b))
            .then_with(|| other_last(&a.region_code).cmp(&other_last(&b.region_code)))
            .then_with(|| a.region_label.cmp(&b.region_label))
            // Favourites first within their Region, then alphabetical.
            .then_with(|| b.is_favourite.cmp(&a.is_favourite))
            .then_with(|| a.name.cmp(&b.name))
    });
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

    fn playable(id: i64, kind: PlayableKind, ref_id: &str) -> PlayableRef {
        PlayableRef {
            provider_id: id,
            kind,
            ref_id: ref_id.into(),
        }
    }

    fn fav(id: i64, kind: FavouriteKind, ref_id: &str) -> FavouriteRef {
        FavouriteRef {
            provider_id: id,
            kind,
            ref_id: ref_id.into(),
        }
    }

    /// Two Episodes of one Series, so Up Next has somewhere to go.
    fn with_series(db: &Db, id: i64) {
        db.replace_episodes(
            id,
            100,
            &[
                Episode {
                    episode_id: "e1".into(),
                    series_id: 100,
                    season: 1,
                    episode_number: 1,
                    title: "Pilot".into(),
                    plot: None,
                    container_extension: Some("mkv".into()),
                    duration_secs: Some(3000),
                    resume: None,
                },
                Episode {
                    episode_id: "e2".into(),
                    series_id: 100,
                    season: 1,
                    episode_number: 2,
                    title: "Second".into(),
                    plot: None,
                    container_extension: Some("mkv".into()),
                    duration_secs: Some(3000),
                    resume: None,
                },
            ],
        )
        .unwrap();
    }

    #[test]
    fn a_channel_never_gets_a_watch_state() {
        let (db, id) = seeded();
        let r = playable(id, PlayableKind::Channel, "42");
        db.save_watch_state(&r, 600, None).unwrap();
        assert!(db.resume_point(&r).unwrap().is_none());
    }

    #[test]
    fn finishing_a_movie_records_it_watched_instead_of_deleting_it() {
        let (db, id) = seeded();
        let r = playable(id, PlayableKind::Movie, "7");
        db.save_watch_state(&r, 300, Some(6000)).unwrap();
        assert!(db.resume_point(&r).unwrap().is_some());

        db.save_watch_state(&r, 5900, Some(6000)).unwrap();
        assert!(
            db.resume_point(&r).unwrap().is_none(),
            "no longer resumable"
        );

        // The row survives — this is what ADR-0006 exists for.
        let state: String = db
            .conn()
            .query_row(
                "SELECT state FROM watch_state WHERE provider_id=?1 AND kind='movie' AND ref_id='7'",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "watched");
    }

    #[test]
    fn the_first_thirty_seconds_are_not_worth_resuming() {
        let (db, id) = seeded();
        let r = playable(id, PlayableKind::Movie, "7");
        db.save_watch_state(&r, 12, Some(6000)).unwrap();
        assert!(db.resume_point(&r).unwrap().is_none());
    }

    #[test]
    fn favourites_toggle_both_ways() {
        let (db, id) = seeded();
        let r = fav(id, FavouriteKind::Channel, "42");
        assert!(db.toggle_favourite(&r).unwrap());
        assert!(db.channels(id, None, 10, 0).unwrap()[0].is_favourite);
        assert_eq!(db.favourites(id).unwrap().channels.len(), 1);
        assert!(!db.toggle_favourite(&r).unwrap());
        assert!(!db.channels(id, None, 10, 0).unwrap()[0].is_favourite);
        assert_eq!(db.favourites(id).unwrap().channels.len(), 0);
    }

    #[test]
    fn a_series_can_be_favourited_even_though_it_is_not_playable() {
        let (db, id) = seeded();
        assert!(db
            .toggle_favourite(&fav(id, FavouriteKind::Series, "100"))
            .unwrap());
    }

    // ---- Up Next (ADR-0006) ---------------------------------------------

    #[test]
    fn up_next_is_the_lowest_unwatched_episode() {
        let (db, id) = seeded();
        with_series(&db, id);
        let next = db.up_next(id, 100).unwrap().unwrap();
        assert_eq!(next.ref_id, "e1");
        assert!(next.is_up_next);
    }

    #[test]
    fn up_next_advances_once_an_episode_is_watched() {
        let (db, id) = seeded();
        with_series(&db, id);
        db.mark_watched(&playable(id, PlayableKind::Episode, "e1"))
            .unwrap();
        assert_eq!(db.up_next(id, 100).unwrap().unwrap().ref_id, "e2");
    }

    #[test]
    fn up_next_is_none_once_every_episode_is_watched() {
        let (db, id) = seeded();
        with_series(&db, id);
        for e in ["e1", "e2"] {
            db.mark_watched(&playable(id, PlayableKind::Episode, e))
                .unwrap();
        }
        assert!(db.up_next(id, 100).unwrap().is_none());
    }

    #[test]
    fn rewatching_an_early_episode_does_not_send_up_next_backwards() {
        let (db, id) = seeded();
        with_series(&db, id);
        for e in ["e1", "e2"] {
            db.mark_watched(&playable(id, PlayableKind::Episode, e))
                .unwrap();
        }
        // Rewatch the first one. Recency would now offer e2, already seen.
        db.save_watch_state(&playable(id, PlayableKind::Episode, "e1"), 100, Some(3000))
            .unwrap();
        assert!(
            db.up_next(id, 100).unwrap().is_none(),
            "everything else is watched; a rewatch must not resurrect e2"
        );
    }

    #[test]
    fn continue_watching_shows_one_row_per_series() {
        let (db, id) = seeded();
        with_series(&db, id);
        // In progress on e1, and e2 is unwatched: the Series must appear once.
        db.save_watch_state(&playable(id, PlayableKind::Episode, "e1"), 100, Some(3000))
            .unwrap();
        let items = db.continue_watching(id, 20).unwrap();
        let episodes: Vec<_> = items.iter().filter(|i| i.series_id == Some(100)).collect();
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].ref_id, "e1");
        assert!(!episodes[0].is_up_next);
    }

    #[test]
    fn continue_watching_offers_up_next_once_the_episode_is_finished() {
        let (db, id) = seeded();
        with_series(&db, id);
        db.save_watch_state(&playable(id, PlayableKind::Episode, "e1"), 2990, Some(3000))
            .unwrap();
        let items = db.continue_watching(id, 20).unwrap();
        let ep = items.iter().find(|i| i.series_id == Some(100)).unwrap();
        assert_eq!(ep.ref_id, "e2");
        assert!(ep.is_up_next);
    }

    #[test]
    fn continue_watching_ignores_a_series_never_started() {
        let (db, id) = seeded();
        with_series(&db, id);
        assert!(db.continue_watching(id, 20).unwrap().is_empty());
    }

    // ---- Reconciliation (ADR-0007) --------------------------------------

    #[test]
    fn a_renumbered_id_drops_the_favourite_instead_of_aliasing_it() {
        let (db, id) = seeded();
        db.toggle_favourite(&fav(id, FavouriteKind::Channel, "42"))
            .unwrap();

        // The Provider reassigns id 42 to a completely different Channel.
        db.replace_catalogue(
            id,
            CatalogueBatch {
                categories: vec![(CatalogueKind::Live, 1, "Spain".into())],
                channels: vec![Channel {
                    stream_id: 42,
                    name: "FRANCE | TF1".into(),
                    icon: None,
                    category_id: Some(1),
                    channel_number: Some(1),
                    has_archive: false,
                    epg_channel_id: None,
                    is_favourite: false,
                }],
                movies: vec![],
                series: vec![],
            },
        )
        .unwrap();

        assert_eq!(
            db.favourites(id).unwrap().channels.len(),
            0,
            "the star must not survive onto a different Channel"
        );
    }

    #[test]
    fn a_favourite_survives_a_sync_that_changes_nothing() {
        let (db, id) = seeded();
        db.toggle_favourite(&fav(id, FavouriteKind::Channel, "42"))
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
                movies: vec![],
                series: vec![],
            },
        )
        .unwrap();
        assert_eq!(db.favourites(id).unwrap().channels.len(), 1);
    }

    #[test]
    fn a_full_sync_does_not_wipe_episode_watch_state() {
        let (db, id) = seeded();
        with_series(&db, id);
        db.save_watch_state(&playable(id, PlayableKind::Episode, "e1"), 100, Some(3000))
            .unwrap();

        // A full Sync never fetches Episodes, so reconciling them here would
        // destroy the Watch State of every Series the Viewer has open.
        db.replace_catalogue(id, CatalogueBatch::default()).unwrap();

        assert!(db
            .resume_point(&playable(id, PlayableKind::Episode, "e1"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn categories_carry_their_item_count() {
        let (db, id) = seeded();
        let cats = db.categories(id, CatalogueKind::Live).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].count, 1);
    }

    // ---- Regions (ADR-0008) ---------------------------------------------

    fn with_regions(db: &Db, id: i64) {
        db.replace_catalogue(
            id,
            CatalogueBatch {
                categories: vec![
                    (CatalogueKind::Live, 1, "ES - FUTBOL".into()),
                    (CatalogueKind::Live, 2, "ES - CINE".into()),
                    (CatalogueKind::Live, 3, "UK - SPORTS".into()),
                    (CatalogueKind::Live, 4, "NBA".into()),
                ],
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn categories_are_tagged_with_a_region_at_sync() {
        let (db, id) = seeded();
        with_regions(&db, id);
        let cats = db.categories(id, CatalogueKind::Live).unwrap();
        let by_name = |n: &str| cats.iter().find(|c| c.name == n).unwrap();
        assert_eq!(by_name("ES - FUTBOL").region_code, "ES");
        assert_eq!(by_name("UK - SPORTS").region_code, "GB");
        assert_eq!(by_name("NBA").region_code, region::OTHER);
        assert_eq!(by_name("NBA").region_label, "Other");
    }

    #[test]
    fn everything_is_visible_before_the_viewer_curates() {
        let (db, id) = seeded();
        with_regions(&db, id);
        assert_eq!(db.categories(id, CatalogueKind::Live).unwrap().len(), 4);
        assert!(db.regions(id).unwrap().iter().all(|r| r.visible));
    }

    #[test]
    fn hiding_a_region_removes_its_categories_from_the_rail() {
        let (db, id) = seeded();
        with_regions(&db, id);
        db.set_region_visible(id, "GB", false).unwrap();
        let names: Vec<_> = db
            .categories(id, CatalogueKind::Live)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(!names.contains(&"UK - SPORTS".to_string()));
        assert!(names.contains(&"ES - FUTBOL".to_string()));
    }

    #[test]
    fn a_region_new_since_curation_arrives_hidden_and_is_counted() {
        let (db, id) = seeded();
        with_regions(&db, id);
        db.set_region_visible(id, "GB", false).unwrap(); // curates

        // A later Sync introduces Italy.
        db.replace_catalogue(
            id,
            CatalogueBatch {
                categories: vec![
                    (CatalogueKind::Live, 1, "ES - FUTBOL".into()),
                    (CatalogueKind::Live, 9, "IT - SKY ITALIA".into()),
                ],
                ..Default::default()
            },
        )
        .unwrap();

        let names: Vec<_> = db
            .categories(id, CatalogueKind::Live)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(
            !names.contains(&"IT - SKY ITALIA".to_string()),
            "a Region new since curation must not reappear on its own"
        );
        assert_eq!(db.new_region_count(id).unwrap(), 1, "and must be reported");
    }

    #[test]
    fn favourite_categories_come_first_within_their_region() {
        let (db, id) = seeded();
        with_regions(&db, id);
        db.set_region_order(id, &["ES".into(), "GB".into()])
            .unwrap();
        db.toggle_favourite(&fav(id, FavouriteKind::Category, "live:2"))
            .unwrap(); // ES - CINE

        let names: Vec<_> = db
            .categories(id, CatalogueKind::Live)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        // ES first because ordered so; CINE before FUTBOL because starred,
        // despite being second alphabetically. Other last.
        assert_eq!(
            names,
            vec!["ES - CINE", "ES - FUTBOL", "UK - SPORTS", "NBA"]
        );
    }

    #[test]
    fn a_viewer_override_survives_a_sync() {
        let (db, id) = seeded();
        with_regions(&db, id);
        db.set_category_region(id, CatalogueKind::Live, 4, Some("US"))
            .unwrap(); // NBA is American, actually

        with_regions(&db, id); // full re-Sync wipes and rewrites categories

        let cats = db.categories(id, CatalogueKind::Live).unwrap();
        let nba = cats.iter().find(|c| c.name == "NBA").unwrap();
        assert_eq!(
            nba.region_code, "US",
            "the correction must outlive the wipe"
        );
    }
}
