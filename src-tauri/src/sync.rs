//! **Sync** — refreshing the local mirror from a **Provider**.
//!
//! Only ever runs when the **Viewer** asks (ADR-0004). Progress is pushed to
//! the webview as `sync:progress` events so a 50MB fetch has a visible pulse.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::model::{Channel, Episode, Movie, Programme, Series};
use crate::db::{CatalogueBatch, Db};
use crate::error::Result;
use crate::xtream::de::maybe_base64;
use crate::xtream::{CatalogueKind, XtreamClient};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub provider_id: i64,
    /// `"categories"`, `"channels"`, `"movies"`, `"series"`, `"saving"`, `"done"`
    pub stage: &'static str,
    pub items: usize,
}

fn report(app: &AppHandle, provider_id: i64, stage: &'static str, items: usize) {
    let _ = app.emit(
        "sync:progress",
        SyncProgress {
            provider_id,
            stage,
            items,
        },
    );
}

/// Pulls a whole **Catalogue** and replaces the mirror in one transaction.
///
/// **Episodes** are excluded on purpose: `get_series_info` is one request per
/// **Series**, so a 5000-series provider would mean 5000 round trips. They are
/// fetched lazily when a Series is opened.
pub async fn sync_catalogue(
    app: &AppHandle,
    db: &Db,
    client: &XtreamClient,
    provider_id: i64,
) -> Result<CatalogueBatch> {
    let entitlement = client.handshake().await?;
    db.record_entitlement(
        provider_id,
        entitlement.user_info.status.as_deref(),
        entitlement.user_info.exp_date,
        entitlement.user_info.max_connections,
    )?;

    let mut batch = CatalogueBatch::default();

    for kind in [
        CatalogueKind::Live,
        CatalogueKind::Movie,
        CatalogueKind::Series,
    ] {
        for c in client.categories(kind).await? {
            let name = c
                .category_name
                .unwrap_or_else(|| format!("#{}", c.category_id));
            batch.categories.push((kind, c.category_id, name));
        }
    }
    report(app, provider_id, "categories", batch.categories.len());

    batch.channels = client
        .channels()
        .await?
        .into_iter()
        .filter_map(|c| {
            Some(Channel {
                stream_id: c.stream_id,
                // A nameless row is unusable in a list and unsearchable; drop it.
                name: c.name?,
                icon: c.stream_icon,
                category_id: c.category_id,
                channel_number: c.num,
                has_archive: c.tv_archive,
                epg_channel_id: c.epg_channel_id,
                is_favourite: false,
            })
        })
        .collect();
    report(app, provider_id, "channels", batch.channels.len());

    batch.movies = client
        .movies()
        .await?
        .into_iter()
        .filter_map(|m| {
            Some(Movie {
                stream_id: m.stream_id,
                name: m.name?,
                icon: m.stream_icon,
                category_id: m.category_id,
                container_extension: m.container_extension,
                rating: m.rating,
                added_at: m.added,
                is_favourite: false,
                resume: None,
            })
        })
        .collect();
    report(app, provider_id, "movies", batch.movies.len());

    batch.series = client
        .series()
        .await?
        .into_iter()
        .filter_map(|s| {
            Some(Series {
                series_id: s.series_id,
                name: s.name?,
                cover: s.cover,
                plot: s.plot,
                category_id: s.category_id,
                rating: s.rating,
            })
        })
        .collect();
    report(app, provider_id, "series", batch.series.len());

    Ok(batch)
}

/// Fetches one **Series**' **Episodes**. Called when a Series is opened.
pub async fn sync_episodes(
    db: &Db,
    client: &XtreamClient,
    provider_id: i64,
    series_id: i64,
) -> Result<Vec<Episode>> {
    let info = client.series_info(series_id).await?;

    let mut episodes = Vec::new();
    for (season_key, raw) in info.episodes {
        // Panels send the season's episodes as a list, or as an object keyed by
        // episode number. Normalise both to a list.
        let items: Vec<serde_json::Value> = match raw {
            serde_json::Value::Array(a) => a,
            serde_json::Value::Object(o) => o.into_values().collect(),
            _ => continue,
        };
        let season_from_key: i64 = season_key.trim().parse().unwrap_or(0);

        for item in items {
            let Ok(e) = serde_json::from_value::<crate::xtream::model::RawEpisode>(item) else {
                continue;
            };
            let Some(episode_id) = e.id else { continue };
            let info = e.info;
            episodes.push(Episode {
                episode_id,
                series_id,
                season: e.season.unwrap_or(season_from_key),
                episode_number: e.episode_num.unwrap_or(0),
                title: e.title.unwrap_or_else(|| "Untitled".into()),
                plot: info.as_ref().and_then(|i| i.plot.clone()),
                container_extension: e.container_extension,
                duration_secs: info.as_ref().and_then(|i| i.duration_secs),
                resume: None,
            });
        }
    }

    episodes.sort_by_key(|e| (e.season, e.episode_number));
    db.replace_episodes(provider_id, series_id, &episodes)?;
    db.episodes(provider_id, series_id)
}

/// Fetches the next few **Programmes** for one **Channel**.
pub async fn sync_schedule(
    db: &Db,
    client: &XtreamClient,
    provider_id: i64,
    stream_id: i64,
) -> Result<Vec<Programme>> {
    let raw = client.short_schedule(stream_id, 12).await?;

    let programmes: Vec<Programme> = raw
        .into_iter()
        .filter_map(|p| {
            Some(Programme {
                start_ts: p.start_timestamp?,
                stop_ts: p.stop_timestamp?,
                title: maybe_base64(&p.title?),
                description: p.description.as_deref().map(maybe_base64),
            })
        })
        .collect();

    db.replace_schedule(provider_id, stream_id, &programmes)?;
    Ok(programmes)
}
