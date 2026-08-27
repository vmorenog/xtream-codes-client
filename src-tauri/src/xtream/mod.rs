pub mod client;
pub mod de;
pub mod model;

pub use client::XtreamClient;

use serde::{Deserialize, Serialize};

/// The three sections of a **Catalogue**. Categories never cross these — the
/// API returns a separate category set per kind (see CONTEXT.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogueKind {
    Live,
    Movie,
    Series,
}

impl CatalogueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CatalogueKind::Live => "live",
            CatalogueKind::Movie => "movie",
            CatalogueKind::Series => "series",
        }
    }

    fn category_action(self) -> &'static str {
        match self {
            CatalogueKind::Live => "get_live_categories",
            CatalogueKind::Movie => "get_vod_categories",
            CatalogueKind::Series => "get_series_categories",
        }
    }
}

/// Anything that can be played. A **Series** is not playable; its **Episodes**
/// are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayableKind {
    Channel,
    Movie,
    Episode,
}

impl PlayableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayableKind::Channel => "channel",
            PlayableKind::Movie => "movie",
            PlayableKind::Episode => "episode",
        }
    }

    /// A live **Channel** has no beginning to return to, so it never gets a
    /// **Resume Point**.
    pub fn resumable(self) -> bool {
        !matches!(self, PlayableKind::Channel)
    }

    pub fn as_favourite_kind(self) -> FavouriteKind {
        match self {
            PlayableKind::Channel => FavouriteKind::Channel,
            PlayableKind::Movie => FavouriteKind::Movie,
            PlayableKind::Episode => FavouriteKind::Episode,
        }
    }

    /// The path segment Xtream uses for this kind's **Stream URL**.
    fn url_segment(self) -> &'static str {
        match self {
            PlayableKind::Channel => "live",
            PlayableKind::Movie => "movie",
            PlayableKind::Episode => "series",
        }
    }
}

/// What a **Favourite** can point at.
///
/// Wider than `PlayableKind` because a **Series** is favouritable but not
/// playable — you pin the show, then play an **Episode** of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FavouriteKind {
    Channel,
    Movie,
    Episode,
    Series,
}

impl FavouriteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FavouriteKind::Channel => "channel",
            FavouriteKind::Movie => "movie",
            FavouriteKind::Episode => "episode",
            FavouriteKind::Series => "series",
        }
    }
}
