mod commands;
mod credentials;
mod db;
mod error;
mod player;
mod sync;
mod xtream;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("XTREAM_LOG")
                .unwrap_or_else(|_| "xtream_client_lib=info,warn".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let db = db::Db::open(&data_dir.join("catalogue.sqlite3"))?;

            // The IPC socket is transient state, not user data.
            let runtime_dir = app.path().app_cache_dir()?;
            let player = player::Player::new(&runtime_dir);

            app.manage(AppState { db, player });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::mpv_installed,
            commands::provider_list,
            commands::provider_add,
            commands::provider_remove,
            commands::provider_sync,
            commands::categories,
            commands::channels,
            commands::movies,
            commands::series_list,
            commands::episodes,
            commands::schedule,
            commands::search,
            commands::toggle_favourite,
            commands::is_favourite,
            commands::save_watch_state,
            commands::mark_watched,
            commands::clear_watch_state,
            commands::favourites,
            commands::continue_watching,
            commands::play,
            commands::player_status,
            commands::player_toggle_pause,
            commands::player_seek,
            commands::player_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
