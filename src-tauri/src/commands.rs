use crate::audio::AudioRecorder;
use crate::azure::{openai, speech};
use crate::config::{store, AppConfig};
use crate::input::TextInjector;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tauri::State;
use serde::{Deserialize, Serialize};

// Global lock to prevent concurrent transcription operations
static IS_TRANSCRIBING: AtomicBool = AtomicBool::new(false);

pub struct AppState {
    pub recorder: Arc<Mutex<AudioRecorder>>,
    pub injector: Arc<Mutex<TextInjector>>,
}

#[derive(Debug, Serialize)]
pub struct TranscriptionResult {
    pub original: String,
    pub polished: Option<String>,
    pub final_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionHistoryItem {
    pub original: String,
    pub polished: Option<String>,
    pub final_text: String,
    pub timestamp: u64,
    pub audio_data: Option<Vec<u8>>,
}

const HISTORY_STORE_FILE: &str = "history.json";
const STATS_STORE_FILE: &str = "stats.json";
const WINDOW_STORE_FILE: &str = "window.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyStats {
    pub date: String,           // YYYY-MM-DD format
    pub transcription_count: u32,
    pub total_characters: u32,
    pub total_duration_secs: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub total_transcriptions: u32,
    pub total_characters: u32,
    pub total_duration_secs: f32,
    pub daily_stats: Vec<DailyStats>,  // Last 30 days
}

#[tauri::command]
pub async fn get_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    store::load_config(&app)
}

#[tauri::command]
pub async fn save_config_cmd(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    store::save_config(&app, &config)
}

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>) -> Result<(), String> {
    let mut recorder = state.recorder.lock().await;
    recorder.start_recording()
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let mut recorder = state.recorder.lock().await;
    recorder.stop_recording()
}

#[tauri::command]
pub async fn get_audio_level(state: State<'_, AppState>) -> Result<f32, String> {
    let recorder = state.recorder.lock().await;
    Ok(recorder.get_audio_level())
}

#[tauri::command]
pub async fn transcribe_and_insert(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    audio_data: Vec<u8>,
) -> Result<TranscriptionResult, String> {
    // Prevent concurrent transcription operations
    if IS_TRANSCRIBING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        log::warn!("transcribe_and_insert called while another transcription is in progress - ignoring");
        return Err("Transcription already in progress".to_string());
    }

    // Use a guard to ensure IS_TRANSCRIBING is reset even if we return early
    struct TranscriptionGuard;
    impl Drop for TranscriptionGuard {
        fn drop(&mut self) {
            IS_TRANSCRIBING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = TranscriptionGuard;

    // Load config
    let config = store::load_config(&app)?;

    // Validate Azure credentials
    if config.azure.speech_key.is_empty() {
        return Err("Azure Speech key not configured".to_string());
    }

    // Transcribe audio with retry
    let transcript = speech::transcribe_audio_with_retry(
        audio_data,
        &config.azure.speech_key,
        &config.azure.speech_region,
        &config.language.speech_languages,
        2, // max retries (1 initial + 1 retry)
    )
    .await?;

    log::info!("Transcription: {}", transcript);

    // Optionally polish text
    let (final_text, polished) = if config.features.text_polishing_enabled
        && !config.azure.openai_key.is_empty()
        && !config.azure.openai_endpoint.is_empty()
    {
        log::info!(">>> Text polishing ENABLED - calling Azure OpenAI...");
        println!(">>> Text polishing ENABLED - calling Azure OpenAI...");
        match openai::polish_text(
            &transcript,
            &config.azure.openai_endpoint,
            &config.azure.openai_key,
            &config.azure.openai_deployment,
        )
        .await
        {
            Ok(polished_text) => {
                log::info!(">>> Polished text: {}", polished_text);
                println!(">>> Polished text: {}", polished_text);
                (polished_text.clone(), Some(polished_text))
            }
            Err(e) => {
                log::warn!(">>> Failed to polish text: {}. Using original transcript.", e);
                println!(">>> Failed to polish text: {}. Using original.", e);
                (transcript.clone(), None)
            }
        }
    } else {
        log::info!(">>> Text polishing DISABLED or not configured");
        println!(">>> Text polishing DISABLED or not configured");
        (transcript.clone(), None)
    };

    // Insert into active window if enabled
    if config.features.auto_insert_enabled {
        let mut injector = state.injector.lock().await;
        injector.inject_text(&final_text)?;
    }

    Ok(TranscriptionResult {
        original: transcript,
        polished,
        final_text,
    })
}

#[tauri::command]
pub async fn open_config_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("config") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        tauri::webview::WebviewWindowBuilder::new(
            &app,
            "config",
            tauri::WebviewUrl::App("/config".into()),
        )
        .title("FluxVoice Configuration")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_history_item(
    app: tauri::AppHandle,
    item: TranscriptionHistoryItem,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(HISTORY_STORE_FILE)
        .map_err(|e| format!("Failed to open history store: {}", e))?;

    // Load existing history
    let mut history: Vec<TranscriptionHistoryItem> = store
        .get("history")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Add new item at the beginning
    history.insert(0, item);

    // Save back
    let history_value = serde_json::to_value(&history)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    store.set("history", history_value);

    // Immediately flush to disk
    store
        .save()
        .map_err(|e| format!("Failed to save history store: {}", e))?;

    log::info!("History item saved to disk, total items: {}", history.len());

    Ok(())
}

#[tauri::command]
pub async fn load_history(app: tauri::AppHandle) -> Result<Vec<TranscriptionHistoryItem>, String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(HISTORY_STORE_FILE)
        .map_err(|e| format!("Failed to open history store: {}", e))?;

    let history: Vec<TranscriptionHistoryItem> = store
        .get("history")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    log::info!("Loaded {} history items from disk", history.len());

    Ok(history)
}

#[tauri::command]
pub async fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(HISTORY_STORE_FILE)
        .map_err(|e| format!("Failed to open history store: {}", e))?;

    let empty_history: Vec<TranscriptionHistoryItem> = vec![];
    let history_value = serde_json::to_value(&empty_history)
        .map_err(|e| format!("Failed to serialize empty history: {}", e))?;

    store.set("history", history_value);

    store
        .save()
        .map_err(|e| format!("Failed to save history store: {}", e))?;

    log::info!("History cleared");

    Ok(())
}

fn get_today_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Simple date calculation (not accounting for timezones perfectly, but good enough)
    let days = secs / 86400;
    let year = 1970 + (days / 365); // Approximate
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
}

#[tauri::command]
pub async fn update_stats(
    app: tauri::AppHandle,
    characters: u32,
    duration_secs: f32,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(STATS_STORE_FILE)
        .map_err(|e| format!("Failed to open stats store: {}", e))?;

    // Load existing stats
    let mut stats: UsageStats = store
        .get("stats")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Update totals
    stats.total_transcriptions += 1;
    stats.total_characters += characters;
    stats.total_duration_secs += duration_secs;

    // Update daily stats
    let today = get_today_date();
    if let Some(daily) = stats.daily_stats.iter_mut().find(|d| d.date == today) {
        daily.transcription_count += 1;
        daily.total_characters += characters;
        daily.total_duration_secs += duration_secs;
    } else {
        stats.daily_stats.push(DailyStats {
            date: today,
            transcription_count: 1,
            total_characters: characters,
            total_duration_secs: duration_secs,
        });
    }

    // Keep only last 30 days
    if stats.daily_stats.len() > 30 {
        stats.daily_stats = stats.daily_stats.into_iter().rev().take(30).rev().collect();
    }

    // Save back
    let stats_value = serde_json::to_value(&stats)
        .map_err(|e| format!("Failed to serialize stats: {}", e))?;

    store.set("stats", stats_value);

    store
        .save()
        .map_err(|e| format!("Failed to save stats store: {}", e))?;

    log::info!(
        "Stats updated: {} transcriptions, {} chars total",
        stats.total_transcriptions,
        stats.total_characters
    );

    Ok(())
}

#[tauri::command]
pub async fn get_stats(app: tauri::AppHandle) -> Result<UsageStats, String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(STATS_STORE_FILE)
        .map_err(|e| format!("Failed to open stats store: {}", e))?;

    let stats: UsageStats = store
        .get("stats")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    Ok(stats)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[tauri::command]
pub async fn save_window_position(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(WINDOW_STORE_FILE)
        .map_err(|e| format!("Failed to open window store: {}", e))?;

    let position = WindowPosition { x, y };
    let position_value = serde_json::to_value(&position)
        .map_err(|e| format!("Failed to serialize position: {}", e))?;

    store.set("position", position_value);

    store
        .save()
        .map_err(|e| format!("Failed to save window store: {}", e))?;

    log::info!("Window position saved: ({}, {})", x, y);

    Ok(())
}

#[tauri::command]
pub async fn load_window_position(app: tauri::AppHandle) -> Result<Option<WindowPosition>, String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(WINDOW_STORE_FILE)
        .map_err(|e| format!("Failed to open window store: {}", e))?;

    let position: Option<WindowPosition> = store
        .get("position")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    Ok(position)
}
