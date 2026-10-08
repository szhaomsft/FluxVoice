use crate::audio::AudioRecorder;
use crate::azure::{openai, speech};
use crate::config::{store, AppConfig, SttModel};
use crate::input::TextInjector;
use crate::hotkey::{parse_hotkey, HotkeyManager};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tauri::{Manager, State};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Deserialize)]
pub struct LatencyStage {
    pub stage: String,
    pub duration_ms: f64,
}

#[tauri::command]
pub fn report_latency(stages: Vec<LatencyStage>, success: bool) {
    println!("[latency] frontend_summary success={}", success);
    for stage in stages {
        println!("[latency] {}={:.1}", stage.stage, stage.duration_ms);
    }
}

// Global lock to prevent concurrent transcription operations
static IS_TRANSCRIBING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn get_build_commit() -> &'static str {
    env!("FLUXVOICE_BUILD_COMMIT")
}

pub struct AppState {
    pub recorder: Arc<Mutex<AudioRecorder>>,
    pub injector: Arc<Mutex<TextInjector>>,
    pub screen_context: Mutex<Option<Result<crate::screen_context::Capture, String>>>,
    pub recording_config: Mutex<Option<AppConfig>>,
}

#[derive(Debug, Serialize)]
pub struct TranscriptionResult {
    pub original: String,
    pub polished: Option<String>,
    pub final_text: String,
    pub post_processing_mode: String,
    pub warning: Option<String>,
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
    speech::validate_model_settings(&config.language)?;
    let (modifiers, key) = parse_hotkey(&config.hotkey)?;
    let manager = app.try_state::<Arc<Mutex<HotkeyManager>>>()
        .ok_or_else(|| "Recording shortcut manager is unavailable".to_string())?;
    let mut manager = manager.lock().await;
    let previous = store::load_config(&app)?;
    let changed = previous.hotkey != config.hotkey;
    if changed {
        manager.register(modifiers, key).await?;
    }
    if let Err(error) = store::save_config(&app, &config) {
        let mut rollback_errors = Vec::new();
        if changed {
            let rollback = match parse_hotkey(&previous.hotkey) {
                Ok((modifiers, key)) => manager.register(modifiers, key).await,
                Err(error) => Err(error),
            };
            if let Err(rollback_error) = rollback {
                log::error!("Failed to restore recording shortcut: {}", rollback_error);
                rollback_errors.push(rollback_error);
            }
        }
        if let Err(rollback_error) = store::save_config(&app, &previous) {
            log::error!("Failed to restore previous configuration: {}", rollback_error);
            rollback_errors.push(rollback_error);
        }
        return Err(if rollback_errors.is_empty() {
            error
        } else {
            format!("{}; rollback failed: {}", error, rollback_errors.join("; "))
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn start_recording(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = store::load_config(&app)?;
    speech::validate_model_settings(&config.language)?;
    let mut screen_context = state.screen_context.lock().await;
    *screen_context = None;
    let capture = if config.features.screen_phrase_hints_enabled {
        Some(crate::screen_context::start_capture())
    } else {
        None
    };
    let mut recorder = state.recorder.lock().await;
    recorder.start_recording()?;
    *state.recording_config.lock().await = Some(config.clone());
    *screen_context = capture;
    drop(screen_context);
    drop(recorder);
    tauri::async_runtime::spawn(async move {
        let openai_endpoint = if config.features.post_processing_mode != "none"
            && !config.azure.openai_key.is_empty()
            && !config.azure.openai_endpoint.is_empty()
        {
            Some(config.azure.openai_endpoint.as_str())
        } else {
            None
        };
        if !config.azure.speech_key.is_empty() {
            crate::azure::warm_connections(&config.azure.speech_region, openai_endpoint).await;
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let started = Instant::now();
    let mut recorder = state.recorder.lock().await;
    println!("[latency] recorder_lock_wait_ms={:.1}", started.elapsed().as_secs_f64() * 1000.0);
    let use_mp3 = state.recording_config.lock().await.as_ref()
        .is_some_and(|config| config.language.stt_model == SttModel::MaiTranscribe2);
    let result = recorder.stop_recording(use_mp3);
    drop(recorder);
    if result.is_err() {
        state.screen_context.lock().await.take();
        state.recording_config.lock().await.take();
    }
    println!("[latency] stop_command_ms={:.1} success={}", started.elapsed().as_secs_f64() * 1000.0, result.is_ok());
    result
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
    let capture = state.screen_context.lock().await.take();

    // Load config
    let config_started = Instant::now();
    let config = match state.recording_config.lock().await.take() {
        Some(config) => config,
        None => store::load_config(&app)?,
    };
    println!("[latency] config_load_ms={:.1}", config_started.elapsed().as_secs_f64() * 1000.0);

    // Validate Azure credentials
    if config.azure.speech_key.is_empty() {
        return Err("Azure Speech key not configured".to_string());
    }

    let mut warnings = Vec::new();
    let phrases = if config.features.screen_phrase_hints_enabled {
        let result = match capture {
            Some(Ok(capture)) => capture.finish().await,
            Some(Err(error)) => Err(error),
            None => Ok(Vec::new()),
        };
        match result {
            Ok(phrases) => phrases,
            Err(error) => {
                log::warn!("Screen phrase hints unavailable: {}", error);
                warnings.push(format!("Screen phrase hints unavailable: {}", error));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Transcribe audio with retry
    let speech_started = Instant::now();
    let transcript_result = speech::transcribe_audio_with_retry(
        audio_data,
        &config.azure.speech_key,
        &config.azure.speech_region,
        &config.language,
        &phrases,
        2, // max retries (1 initial + 1 retry)
    )
    .await;
    println!("[latency] speech_total_including_retries_ms={:.1} success={}", speech_started.elapsed().as_secs_f64() * 1000.0, transcript_result.is_ok());
    let transcript = transcript_result?;

    log::info!("Transcription: {}", transcript);

    // Post-process based on mode: none, polish, or translate
    let mode = config.features.post_processing_mode.clone();
    log::info!(">>> Post-processing mode from config: '{}'", mode);
    println!(">>> Post-processing mode from config: '{}'", mode);

    let processing_started = Instant::now();
    let (final_text, polished) = if !config.azure.openai_key.is_empty()
        && !config.azure.openai_endpoint.is_empty()
    {
        match mode.as_str() {
            "polish" => {
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
                        warnings.push(format!("Polish failed: {}", e));
                        (transcript.clone(), None)
                    }
                }
            }
            "translate" => {
                let target_lang = &config.features.translate_target_language;
                log::info!(">>> Translation ENABLED - translating to {} via Azure OpenAI...", target_lang);
                println!(">>> Translation ENABLED - translating to {} via Azure OpenAI...", target_lang);
                match openai::translate_text(
                    &transcript,
                    target_lang,
                    &config.azure.openai_endpoint,
                    &config.azure.openai_key,
                    &config.azure.openai_deployment,
                )
                .await
                {
                    Ok(translated_text) => {
                        log::info!(">>> Translated text: {}", translated_text);
                        println!(">>> Translated text: {}", translated_text);
                        (translated_text.clone(), Some(translated_text))
                    }
                    Err(e) => {
                        log::warn!(">>> Failed to translate text: {}. Using original transcript.", e);
                        println!(">>> Failed to translate text: {}. Using original.", e);
                        warnings.push(format!("Translation failed: {}", e));
                        (transcript.clone(), None)
                    }
                }
            }
            _ => {
                log::info!(">>> Post-processing mode: none");
                println!(">>> Post-processing mode: none");
                (transcript.clone(), None)
            }
        }
    } else {
        log::info!(">>> OpenAI not configured - skipping post-processing");
        println!(">>> OpenAI not configured - skipping post-processing");
        (transcript.clone(), None)
    };

    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };
    println!("[latency] post_processing_ms={:.1} mode={} warning={}", processing_started.elapsed().as_secs_f64() * 1000.0, mode, warning.is_some());

    // Insert into active window if enabled
    if config.features.auto_insert_enabled {
        let insertion_started = Instant::now();
        let mut injector = state.injector.lock().await;
        let insertion_result = injector.inject_text(&final_text);
        println!("[latency] text_insertion_ms={:.1} success={}", insertion_started.elapsed().as_secs_f64() * 1000.0, insertion_result.is_ok());
        insertion_result?;
    }

    Ok(TranscriptionResult {
        original: transcript,
        polished,
        final_text,
        post_processing_mode: mode,
        warning,
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
    let started = Instant::now();
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
    println!("[latency] background_history_save_ms={:.1}", started.elapsed().as_secs_f64() * 1000.0);

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
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tauri::command]
pub async fn update_stats(
    app: tauri::AppHandle,
    characters: u32,
    duration_secs: f32,
) -> Result<(), String> {
    let started = Instant::now();
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
    println!("[latency] background_stats_save_ms={:.1}", started.elapsed().as_secs_f64() * 1000.0);

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
