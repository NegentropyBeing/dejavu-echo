use crate::config::Settings;
use crate::discord::{self, DiscordEvent, Outgoing, WebhookState};
use crate::engine::{EngineState, Outcome};
use crate::model::{Counters, EngineStatus, Moderation, SkipReason};
use crate::secrets::{self, Secrets};
use crate::twitch::{self, Event as TwitchEvent};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;

const SETTINGS_FILE: &str = "settings.json";
const LOG_CAPACITY: usize = 500;
const DISCORD_WEBHOOK_SETTINGS: &str = "https://discord.com/settings";
const TICK: Duration = Duration::from_millis(250);

/// Onde fica o arquivo oculto com o webhook.
#[derive(Debug, Clone)]
pub struct ConfigDir(pub std::path::PathBuf);

struct Running {
    channel: String,
    stop: Arc<AtomicBool>,
    /// Mantido vivo enquanto o motor roda: ao ser descartado, a conexão IRC fecha.
    #[allow(dead_code, reason = "o descarte é o que encerra a conexão")]
    client: twitch::Client,
}

pub struct AppState {
    settings: Mutex<Settings>,
    status: Mutex<EngineStatus>,
    webhook: Mutex<WebhookState>,
    logs: Mutex<Vec<LogEntry>>,
    counters: Mutex<Counters>,
    running: Mutex<Option<Running>>,
    next_id: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            status: Mutex::new(EngineStatus::Idle),
            webhook: Mutex::new(WebhookState::Unknown),
            logs: Mutex::new(Vec::new()),
            counters: Mutex::new(Counters::default()),
            running: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }
}

impl AppState {
    pub fn set_settings(&self, s: Settings) {
        *self.settings.lock().unwrap() = s;
    }

    pub fn get_settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: u64,
    pub at: String,
    pub level: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub status: EngineStatus,
    pub channel: Option<String>,
    pub counters: Counters,
    pub webhook: WebhookState,
    pub logs: Vec<LogEntry>,
}

impl AppState {
    fn push_log(&self, app: &AppHandle, level: &'static str, text: impl Into<String>) {
        let entry = LogEntry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            level,
            text: text.into(),
        };
        let mut logs = self.logs.lock().unwrap();
        if logs.len() >= LOG_CAPACITY {
            let excess = logs.len() + 1 - LOG_CAPACITY;
            logs.drain(0..excess);
        }
        logs.push(entry.clone());
        let _ = app.emit("log", &entry);
    }

    fn set_status(&self, app: &AppHandle, status: EngineStatus) {
        *self.status.lock().unwrap() = status.clone();
        let _ = app.emit("status", &status);
    }

    fn bump_skipped(&self) {
        self.counters.lock().unwrap().skipped += 1;
    }

    fn sync_counters(&self, app: &AppHandle) {
        let c = self.counters.lock().unwrap().clone();
        let _ = app.emit("counters", &c);
    }
}

fn skip_label(r: SkipReason) -> &'static str {
    match r {
        SkipReason::BlockedUser => "usuário bloqueado",
        SkipReason::RoleNotAllowed => "cargo não permitido",
        SkipReason::Cooldown => "cooldown",
        SkipReason::Duplicate => "link repetido",
        SkipReason::NoApprovedLink => "nenhum link aprovado",
        SkipReason::DeletedByMod => "mensagem apagada pelo mod",
        SkipReason::BannedByMod => "autor banido pelo mod",
    }
}

// ============================================================
// COMANDOS
// ============================================================

#[tauri::command]
pub fn load_settings(state: State<AppState>) -> Settings {
    state.get_settings()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    state.set_settings(settings.clone());
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    let store = app.store(SETTINGS_FILE).map_err(|e| e.to_string())?;
    store.set("settings", value);
    store.save().map_err(|e| e.to_string())?;
    state.push_log(&app, "info", "regras do filtro salvas");
    Ok(settings)
}

#[tauri::command]
pub async fn webhook_status(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: State<'_, ConfigDir>,
    reset: Option<bool>,
) -> Result<WebhookState, String> {
    let s = secrets::read(&dir.0).map_err(|e| e.to_string())?;
    let Some(url) = s.discord_webhook_url else {
        *state.webhook.lock().unwrap() = WebhookState::Unknown;
        return Ok(WebhookState::Unknown);
    };

    let result = discord::check(&url).await;
    *state.webhook.lock().unwrap() = result;

    // webhook apagado no Discord: descarta o salvo para o usuário colar outro
    if result == WebhookState::Invalid && reset.unwrap_or(false) {
        secrets::clear(&dir.0);
        *state.webhook.lock().unwrap() = WebhookState::Unknown;
        state.push_log(
            &app,
            "warn",
            "webhook salvo não é mais válido; informe um novo",
        );
        return Ok(WebhookState::Unknown);
    }

    Ok(result)
}

#[tauri::command]
pub async fn set_webhook(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: State<'_, ConfigDir>,
    url: String,
) -> Result<WebhookState, String> {
    let url = url.trim().to_string();
    if !discord::is_valid_webhook_url(&url) {
        return Err("Isso não parece uma URL de webhook do Discord.".into());
    }

    match discord::check(&url).await {
        WebhookState::Valid => {}
        WebhookState::Invalid => {
            return Err(
                "O Discord não reconheceu esse webhook. Confira se copiou a URL completa.".into(),
            )
        }
        WebhookState::Unreachable => state.push_log(
            &app,
            "warn",
            "não consegui verificar o webhook agora; salvando mesmo assim",
        ),
        WebhookState::Unknown => {}
    }

    secrets::write(
        &dir.0,
        &Secrets {
            discord_webhook_url: Some(url),
        },
    )
    .map_err(|e| e.to_string())?;

    *state.webhook.lock().unwrap() = WebhookState::Valid;
    state.push_log(
        &app,
        "ok",
        "webhook salvo no arquivo oculto \".secret_env\"",
    );
    Ok(WebhookState::Valid)
}

#[tauri::command]
pub fn clear_webhook(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: State<'_, ConfigDir>,
) -> Result<(), String> {
    secrets::clear(&dir.0);
    *state.webhook.lock().unwrap() = WebhookState::Unknown;
    state.push_log(&app, "info", "webhook apagado");
    Ok(())
}

#[tauri::command]
pub async fn start_engine(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: State<'_, ConfigDir>,
    channel_input: String,
) -> Result<(), String> {
    if state.running.lock().unwrap().is_some() {
        return Err("O motor já está em execução.".into());
    }

    let channel = twitch::normalize_channel_input(&channel_input);
    if !twitch::is_valid_channel(&channel) {
        return Err("Nome de canal inválido.".into());
    }

    let s = secrets::read(&dir.0).map_err(|e| e.to_string())?;
    let Some(webhook_url) = s.discord_webhook_url else {
        return Err("Configure o webhook do Discord antes de iniciar.".into());
    };

    match discord::check(&webhook_url).await {
        WebhookState::Valid => {}
        WebhookState::Invalid => {
            secrets::clear(&dir.0);
            *state.webhook.lock().unwrap() = WebhookState::Unknown;
            return Err("O webhook salvo não existe mais. Configure um novo.".into());
        }
        _ => {}
    }

    *state.webhook.lock().unwrap() = WebhookState::Valid;
    *state.counters.lock().unwrap() = Counters::default();
    *state.logs.lock().unwrap() = Vec::new();

    let settings = state.get_settings();
    let stop = Arc::new(AtomicBool::new(false));
    state.set_status(&app, EngineStatus::Connecting);
    state.push_log(&app, "info", format!("Conectando ao chat de #{channel}..."));

    let (tw_tx, mut tw_rx) = tokio::sync::mpsc::unbounded_channel();
    let client = twitch::spawn(channel.clone(), tw_tx);

    let (dc_tx, dc_rx) = tokio::sync::mpsc::unbounded_channel::<Outgoing>();
    let (dcev_tx, mut dcev_rx) = tokio::sync::mpsc::unbounded_channel::<DiscordEvent>();
    let discord_sender = discord::DiscordSender::new(webhook_url, dc_rx, dcev_tx);
    tauri::async_runtime::spawn(discord_sender.run());

    *state.running.lock().unwrap() = Some(Running {
        channel: channel.clone(),
        stop: stop.clone(),
        client,
    });

    let app_rt = app.clone();
    let settings_rt = settings.normalized();

    tauri::async_runtime::spawn(async move {
        let mut engine = EngineState::new(settings_rt);

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            tokio::select! {
                biased;
                ev = tw_rx.recv() => {
                    let Some(ev) = ev else { break };
                    let st = app_rt.state::<AppState>();
                    match ev {
                        TwitchEvent::Joined(ch) => {
                            st.set_status(&app_rt, EngineStatus::Live);
                            st.push_log(
                                &app_rt,
                                "ok",
                                format!("Lendo o chat de #{ch}. Links aprovados vão para o Discord."),
                            );
                        }
                        TwitchEvent::Disconnected(msg) => {
                            st.set_status(&app_rt, EngineStatus::Error(msg.clone()));
                            st.push_log(&app_rt, "error", format!("Conexão encerrada: {msg}"));
                        }
                        TwitchEvent::Moderation(mod_ev) => {
                            let n = engine.cancel(&mod_ev);
                            if n > 0 {
                                let label = match mod_ev {
                                    Moderation::MessageDeleted { .. } => "apagada pelo mod",
                                    Moderation::UserBanned { .. } => "autor banido pelo mod",
                                };
                                st.push_log(&app_rt, "warn", format!("{n} envio(s) cancelados: {label}"));
                                st.sync_counters(&app_rt);
                            }
                        }
                        TwitchEvent::Message(msg) => {
                            let now = Instant::now();
                            let who = if msg.display_name.is_empty() {
                                msg.user_login.clone()
                            } else {
                                msg.display_name.clone()
                            };
                            if let Outcome::Skipped(reason) = engine.accept(&msg, now) {
                                st.bump_skipped();
                                st.push_log(
                                    &app_rt,
                                    "info",
                                    format!("{who}: ignorado ({})", skip_label(reason)),
                                );
                                st.sync_counters(&app_rt);
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(TICK) => {
                    let now = Instant::now();
                    for id in engine.due(now) {
                        if let Some(content) = engine.commit(&id, now) {
                            let _ = dc_tx.send(Outgoing { content });
                        }
                    }
                }
            }
        }

        // descarta o handle: é isso que encerra a conexão IRC
        *app_rt.state::<AppState>().running.lock().unwrap() = None;

        // eventos do Discord: confirmações e erros de envio
        let st = app_rt.state::<AppState>();
        while let Some(ev) = dcev_rx.recv().await {
            match ev {
                DiscordEvent::Sent { content } => {
                    st.push_log(
                        &app_rt,
                        "ok",
                        format!("Enviado ao Discord: {}", discord::sent_summary(&content)),
                    );
                }
                DiscordEvent::HttpError { status } => {
                    st.push_log(&app_rt, "error", format!("Discord respondeu HTTP {status}"));
                }
                DiscordEvent::WebhookGone => st.push_log(
                    &app_rt,
                    "error",
                    "O webhook não existe mais. Configure um novo.",
                ),
                DiscordEvent::NetworkError { message } => {
                    st.push_log(&app_rt, "error", format!("Erro de rede: {message}"));
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn stop_engine(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if let Some(r) = state.running.lock().unwrap().take() {
        r.stop.store(true, Ordering::Relaxed);
        state.push_log(&app, "info", "Encerrando...");
    }
    Ok(())
}

#[tauri::command]
pub fn engine_status(state: State<AppState>) -> StatusPayload {
    let running = state.running.lock().unwrap();
    StatusPayload {
        status: state.status.lock().unwrap().clone(),
        channel: running.as_ref().map(|r| r.channel.clone()),
        counters: state.counters.lock().unwrap().clone(),
        webhook: *state.webhook.lock().unwrap(),
        logs: state.logs.lock().unwrap().clone(),
    }
}

#[tauri::command]
pub fn reveal_settings_file(app: AppHandle, dir: State<'_, ConfigDir>) -> Result<(), String> {
    // garante que existe antes de tentar abrir a pasta
    std::fs::create_dir_all(&dir.0).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.0.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_discord_webhook_settings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(DISCORD_WEBHOOK_SETTINGS, None::<&str>)
        .map_err(|e| e.to_string())
}
