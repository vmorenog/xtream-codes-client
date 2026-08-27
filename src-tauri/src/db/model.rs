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
    /// Always set. `OTHER` when the name gave nothing away.
    pub region_code: String,
    pub region_label: String,
    pub is_favourite: bool,
}

/// A **Region** as Settings shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub code: String,
    pub label: String,
    pub visible: bool,
    pub sort_order: i64,
    /// How many **Categories** across the whole **Catalogue** sit in it.
    pub category_count: i64,
    /// True until the **Viewer** has curated, or for one that arrived in a
    /// later **Sync** and was hidden on arrival (ADR-0008).
    pub is_new: bool,
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

/// Which of the three a **Playable** is in. A **Channel** holds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchState {
    InProgress,
    Watched,
}

impl WatchState {
    pub fn as_str(self) -> &'static str {
        match self {
            WatchState::InProgress => "in_progress",
            WatchState::Watched => "watched",
        }
    }
}

/// One row of **Continue Watching**.
///
/// Either something the **Viewer** is part-way through, or the **Up Next** of a
/// started **Series**. `isUpNext` tells the UI which, so it can say "S2E4" for
/// one and show a progress bar for the other.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueItem {
    pub kind: crate::xtream::PlayableKind,
    pub ref_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub position_secs: Option<i64>,
    pub duration_secs: Option<i64>,
    pub updated_at: i64,
    pub is_up_next: bool,
    /// Present only for an **Episode**.
    pub series_id: Option<i64>,
    pub series_name: Option<String>,
    pub season: Option<i64>,
    pub episode_number: Option<i64>,
}

/// What Home shows above **Continue Watching**.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favourites {
    pub channels: Vec<Channel>,
    pub movies: Vec<Movie>,
    pub series: Vec<Series>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: String,
    pub ref_id: String,
    pub name: String,
}

/// What the UI sends when it wants something played.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayableRef {
    pub provider_id: i64,
    pub kind: crate::xtream::PlayableKind,
    pub ref_id: String,
}

/// What the UI sends when it wants something starred. Wider than `PlayableRef`
/// because a **Series** is favouritable but not playable.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavouriteRef {
    pub provider_id: i64,
    pub kind: crate::xtream::FavouriteKind,
    pub ref_id: String,
}

impl From<&PlayableRef> for FavouriteRef {
    fn from(p: &PlayableRef) -> Self {
        Self {
            provider_id: p.provider_id,
            kind: p.kind.as_favourite_kind(),
            ref_id: p.ref_id.clone(),
        }
    }
}
