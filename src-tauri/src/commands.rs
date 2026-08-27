//! Everything the webview is allowed to do.
//!
//! The frontend has no network access, no filesystem access and no player
//! access of its own (see `capabilities/default.json`) — it can only call these.

use tauri::{AppHandle, State};

use crate::db::model::*;
use crate::db::Db;
use crate::error::{AppError, Result};
use crate::player::{Player, PlayerStatus};
use crate::xtream::{CatalogueKind, XtreamClient};
use crate::{credentials, sync};

pub struct AppState {
    pub db: Db,
    pub player: Player,
}

/// Rebuilds a client for a **Provider**, pulling its password from the Keychain.
fn client_for(state: &AppState, provider_id: i64) -> Result<XtreamClient> {
    let (base_url, username) = state.db.provider_credentials(provider_id)?;
    let password = credentials::load(provider_id)?;
    XtreamClient::new(&base_url, &username, &password)
}

// ---- Setup ---------------------------------------------------------------

/// Whether mpv is present. The UI shows the `brew install mpv` screen when not.
#[tauri::command]
pub fn mpv_installed() -> bool {
    Player::is_installed()
}

// ---- Providers -----------------------------------------------------------

#[tauri::command]
pub fn provider_list(state: State<'_, AppState>) -> Result<Vec<Provider>> {
    state.db.providers()
}

/// Verifies the credentials before storing anything, so a typo never becomes a
/// **Provider** row that fails on every later call.
#[tauri::command]
pub async fn provider_add(
    state: State<'_, AppState>,
    name: String,
    base_url: String,
    username: String,
    password: String,
) -> Result<i64> {
    let client = XtreamClient::new(&base_url, &username, &password)?;
    let entitlement = client.handshake().await?;

    let id = state.db.add_provider(&name, &base_url, &username)?;

    // If the Keychain refuses, roll the row back rather than leave a Provider
    // that can never authenticate.
    if let Err(e) = credentials::store(id, &password) {
        let _ = state.db.remove_provider(id);
        return Err(e);
    }

    state.db.record_entitlement(
        id,
        entitlement.user_info.status.as_deref(),
        entitlement.user_info.exp_date,
        entitlement.user_info.max_connections,
    )?;
    Ok(id)
}

#[tauri::command]
pub fn provider_remove(state: State<'_, AppState>, provider_id: i64) -> Result<()> {
    state.db.remove_provider(provider_id)?;
    credentials::forget(provider_id);
    Ok(())
}

/// A full **Sync**. Only ever called from the Sync button (ADR-0004).
#[tauri::command]
pub async fn provider_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
) -> Result<CatalogueCounts> {
    let client = client_for(&state, provider_id)?;
    let batch = sync::sync_catalogue(&app, &state.db, &client, provider_id).await?;

    let counts = CatalogueCounts {
        channels: batch.channels.len() as i64,
        movies: batch.movies.len() as i64,
        series: batch.series.len() as i64,
    };

    state.db.replace_catalogue(provider_id, batch)?;
    state.db.mark_synced(provider_id)?;
    Ok(counts)
}

// ---- Catalogue -----------------------------------------------------------

#[tauri::command]
pub fn categories(
    state: State<'_, AppState>,
    provider_id: i64,
    kind: CatalogueKind,
) -> Result<Vec<Category>> {
    state.db.categories(provider_id, kind)
}

#[tauri::command]
pub fn channels(
    state: State<'_, AppState>,
    provider_id: i64,
    category_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Channel>> {
    state.db.channels(
        provider_id,
        category_id,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn movies(
    state: State<'_, AppState>,
    provider_id: i64,
    category_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Movie>> {
    state.db.movies(
        provider_id,
        category_id,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn series_list(
    state: State<'_, AppState>,
    provider_id: i64,
    category_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Series>> {
    state.db.series(
        provider_id,
        category_id,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
}

/// **Episodes** are not part of a full **Sync** (one request per **Series**),
/// so they are fetched the first time a Series is opened and cached after that.
#[tauri::command]
pub async fn episodes(
    state: State<'_, AppState>,
    provider_id: i64,
    series_id: i64,
    refresh: Option<bool>,
) -> Result<Vec<Episode>> {
    let cached = state.db.episodes(provider_id, series_id)?;
    if !cached.is_empty() && !refresh.unwrap_or(false) {
        return Ok(cached);
    }
    let client = client_for(&state, provider_id)?;
    sync::sync_episodes(&state.db, &client, provider_id, series_id).await
}

/// The **Schedule** for a **Channel**. Served from the mirror when it still
/// covers now, refetched otherwise — EPG goes stale in hours, not days.
#[tauri::command]
pub async fn schedule(
    state: State<'_, AppState>,
    provider_id: i64,
    stream_id: i64,
) -> Result<Vec<Programme>> {
    let now = crate::db::now();
    let cached = state.db.schedule(provider_id, stream_id, now)?;
    if cached.len() > 2 {
        return Ok(cached);
    }
    let client = client_for(&state, provider_id)?;
    match sync::sync_schedule(&state.db, &client, provider_id, stream_id).await {
        Ok(fresh) => Ok(fresh),
        // A missing Schedule is normal — plenty of Providers ship no EPG at
        // all. Fall back to whatever the mirror has rather than erroring.
        Err(e) => {
            tracing::warn!(error = %e, stream_id, "schedule fetch failed");
            Ok(cached)
        }
    }
}

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    provider_id: i64,
    query: String,
) -> Result<Vec<SearchHit>> {
    state.db.search(provider_id, &query, 60)
}

// ---- Viewing state -------------------------------------------------------

#[tauri::command]
pub fn toggle_favourite(state: State<'_, AppState>, target: FavouriteRef) -> Result<bool> {
    state.db.toggle_favourite(&target)
}

#[tauri::command]
pub fn is_favourite(state: State<'_, AppState>, target: FavouriteRef) -> Result<bool> {
    state.db.is_favourite(&target)
}

/// Progress from the player. Promotes to **Watched** past 95% (ADR-0006).
#[tauri::command]
pub fn save_watch_state(
    state: State<'_, AppState>,
    playable: PlayableRef,
    position_secs: i64,
    duration_secs: Option<i64>,
) -> Result<()> {
    state
        .db
        .save_watch_state(&playable, position_secs, duration_secs)
}

/// "I meant to skip that" — the escape hatch for the one case **Up Next** gets
/// wrong, a deliberately skipped **Episode** it would otherwise offer forever.
#[tauri::command]
pub fn mark_watched(state: State<'_, AppState>, playable: PlayableRef) -> Result<()> {
    state.db.mark_watched(&playable)
}

/// Puts something back into **Continue Watching** after it was marked watched.
#[tauri::command]
pub fn clear_watch_state(state: State<'_, AppState>, playable: PlayableRef) -> Result<()> {
    state.db.clear_watch_state(&playable)
}

// ---- Home ----------------------------------------------------------------

#[tauri::command]
pub fn favourites(state: State<'_, AppState>, provider_id: i64) -> Result<Favourites> {
    state.db.favourites(provider_id)
}

#[tauri::command]
pub fn continue_watching(
    state: State<'_, AppState>,
    provider_id: i64,
    limit: Option<i64>,
) -> Result<Vec<ContinueItem>> {
    state.db.continue_watching(provider_id, limit.unwrap_or(20))
}

// ---- Playback ------------------------------------------------------------

/// Resolves a **Playable** to its **Stream URL** and hands it to mpv.
///
/// The URL is built here and dropped inside `Player::play`. It is never
/// returned to the webview and never logged (ADR-0002).
#[tauri::command]
pub async fn play(state: State<'_, AppState>, playable: PlayableRef) -> Result<()> {
    if !Player::is_installed() {
        return Err(AppError::MpvMissing);
    }

    let client = client_for(&state, playable.provider_id)?;
    let extension = state.db.container_extension(&playable)?;
    let title = state.db.playable_title(&playable)?;
    let start_at = state
        .db
        .resume_point(&playable)?
        .map(|r| r.position_secs)
        .filter(|_| playable.kind.resumable());

    let url = client.stream_url(playable.kind, &playable.ref_id, extension.as_deref())?;
    state.player.play(url.to_string(), title, start_at).await
}

#[tauri::command]
pub async fn player_status(state: State<'_, AppState>) -> Result<PlayerStatus> {
    Ok(state.player.status().await)
}

#[tauri::command]
pub async fn player_toggle_pause(state: State<'_, AppState>) -> Result<()> {
    state.player.toggle_pause().await
}

#[tauri::command]
pub async fn player_seek(state: State<'_, AppState>, seconds: i64) -> Result<()> {
    state.player.seek(seconds).await
}

#[tauri::command]
pub async fn player_stop(state: State<'_, AppState>) -> Result<()> {
    state.player.stop().await
}
