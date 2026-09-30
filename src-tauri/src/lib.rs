pub mod config;
pub mod discord;
pub mod engine;
pub mod filter;
pub mod model;
pub mod secrets;
pub mod state;
pub mod twitch;

use tauri::Manager;
use tauri_plugin_store::StoreExt;

use state::AppState;

const SETTINGS_FILE: &str = "settings.json";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("DEJAVU_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle();
            let dir = handle
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&dir).ok();
            handle.manage(state::ConfigDir(dir));

            // as regras do filtro são o estado inicial do motor
            let store = handle.store(SETTINGS_FILE)?;
            let loaded: config::Settings = store
                .get("settings")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            app.state::<AppState>().set_settings(loaded);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state::load_settings,
            state::save_settings,
            state::webhook_status,
            state::set_webhook,
            state::clear_webhook,
            state::start_engine,
            state::stop_engine,
            state::engine_status,
            state::reveal_settings_file,
            state::open_discord_webhook_settings,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o aplicativo");
}
