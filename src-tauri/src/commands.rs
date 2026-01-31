use crate::audio::AudioRecorder;
use crate::azure::{openai, speech};
use crate::config::{store, AppConfig};
use crate::input::TextInjector;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tauri::State;
use serde::Serialize;

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
        &config.language.speech_language,
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
