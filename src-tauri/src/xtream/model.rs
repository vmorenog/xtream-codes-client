//! Wire types. These mirror what Xtream panels actually send, not what we want
//! to store — the shapes in `db` are the clean ones.
//!
//! Fields we do not read yet are kept deliberately: they document what a panel
//! actually sends, and deleting them means rediscovering it from a packet
//! capture later.
#![allow(dead_code)]

use super::de::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Handshake {
    pub user_info: UserInfo,
    #[serde(default)]
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    #[serde(default, deserialize_with = "flex_bool")]
    pub auth: bool,
    #[serde(default, deserialize_with = "flex_string")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub exp_date: Option<i64>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub max_connections: Option<i64>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub active_cons: Option<i64>,
    #[serde(default, deserialize_with = "flex_vec")]
    pub allowed_output_formats: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    #[serde(default, deserialize_with = "flex_string")]
    pub https_port: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawCategory {
    #[serde(deserialize_with = "flex_i64_req")]
    pub category_id: i64,
    #[serde(default, deserialize_with = "flex_string")]
    pub category_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawChannel {
    #[serde(deserialize_with = "flex_i64_req")]
    pub stream_id: i64,
    #[serde(default, deserialize_with = "flex_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub stream_icon: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub epg_channel_id: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub num: Option<i64>,
    #[serde(default, deserialize_with = "flex_bool")]
    pub tv_archive: bool,
}

#[derive(Debug, Deserialize)]
pub struct RawMovie {
    #[serde(deserialize_with = "flex_i64_req")]
    pub stream_id: i64,
    #[serde(default, deserialize_with = "flex_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub stream_icon: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "flex_string")]
    pub container_extension: Option<String>,
    #[serde(default, deserialize_with = "flex_f64")]
    pub rating: Option<f64>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub added: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RawSeries {
    #[serde(deserialize_with = "flex_i64_req")]
    pub series_id: i64,
    #[serde(default, deserialize_with = "flex_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub cover: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub plot: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub category_id: Option<i64>,
    #[serde(default, deserialize_with = "flex_f64")]
    pub rating: Option<f64>,
}

/// `get_series_info` nests episodes under a season-number-keyed map whose keys
/// are strings, and whose values some panels send as a map instead of a list.
#[derive(Debug, Deserialize)]
pub struct RawSeriesInfo {
    #[serde(default)]
    pub episodes: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RawEpisode {
    #[serde(default, deserialize_with = "flex_string")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub episode_num: Option<i64>,
    #[serde(default, deserialize_with = "flex_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub container_extension: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub season: Option<i64>,
    #[serde(default)]
    pub info: Option<RawEpisodeInfo>,
}

#[derive(Debug, Deserialize)]
pub struct RawEpisodeInfo {
    #[serde(default, deserialize_with = "flex_string")]
    pub plot: Option<String>,
    /// Panels send this as "45", "45:12" or seconds. We only trust plain ints.
    #[serde(default, deserialize_with = "flex_i64")]
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RawScheduleResponse {
    #[serde(default, deserialize_with = "flex_vec")]
    pub epg_listings: Vec<RawProgramme>,
}

#[derive(Debug, Deserialize)]
pub struct RawProgramme {
    /// base64 in practice, plain text on some panels. See `maybe_base64`.
    #[serde(default, deserialize_with = "flex_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub start_timestamp: Option<i64>,
    #[serde(default, deserialize_with = "flex_i64")]
    pub stop_timestamp: Option<i64>,
}
