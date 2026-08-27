//! Shapes handed to the webview. These are the clean domain types from
//! CONTEXT.md, not the wire types in `xtream::model`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub username: String,
    /// Seconds since epoch, or `None` if this Provider has never been synced.
    pub last_synced_at: Option<i64>,
    pub entitlement: Entitlement,
    pub counts: CatalogueCounts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlement {
    /// `"Active"`, `"Expired"`, `"Banned"`, or `None` before the first check.
    pub status: Option<String>,
    pub expires_at: Option<i64>,
    /// How many **Sessions** may run at once.
    pub max_sessions: Option<i64>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueCounts {
    pub channels: i64,
    pub movies: i64,
    pub series: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub stream_id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub category_id: Option<i64>,
    pub channel_number: Option<i64>,
    pub has_archive: bool,
    /// Ties this **Channel** to its XMLTV **Schedule** feed. Often absent.
    pub epg_channel_id: Option<String>,
    pub is_favourite: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    pub stream_id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub category_id: Option<i64>,
    pub container_extension: Option<String>,
    pub rating: Option<f64>,
    pub added_at: Option<i64>,
    pub is_favourite: bool,
    pub resume: Option<ResumePoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub series_id: i64,
    pub name: String,
    pub cover: Option<String>,
    pub plot: Option<String>,
    pub category_id: Option<i64>,
    pub rating: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub episode_id: String,
    pub series_id: i64,
    pub season: i64,
    pub episode_number: i64,
    pub title: String,
    pub plot: Option<String>,
    pub container_extension: Option<String>,
    pub duration_secs: Option<i64>,
    pub resume: Option<ResumePoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Programme {
    pub start_ts: i64,
    pub stop_ts: i64,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumePoint {
    pub position_secs: i64,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String,
    pub ref_id: String,
    pub name: String,
}

/// What the UI sends when it wants something played or favourited.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayableRef {
    pub provider_id: i64,
    pub kind: crate::xtream::PlayableKind,
    pub ref_id: String,
}
