mod audio;
mod azure;
mod config;
mod commands;
mod history_export;
mod hotkey;
mod input;

use crate::audio::AudioRecorder;
use crate::commands::AppState;
use crate::config::store;
use crate::hotkey::{parse_hotkey, HotkeyManager};
use crate::input::TextInjector;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Initialize app state
            let recorder = Arc::new(Mutex::new(
                AudioRecorder::new()
                    .unwrap_or_else(|e| {
                        log::warn!("Audio recorder init failed (no mic?): {e}. Recording will be unavailable.");
                        AudioRecorder::new_dummy()
                    }),
            ));
            let injector = Arc::new(Mutex::new(TextInjector::new()));

            app.manage(AppState { recorder, injector, recording_config: Mutex::new(None) });

            // Position main window
            if let Some(window) = app.get_webview_window("main") {
                let app_handle_pos = app.handle().clone();
                let window_clone = window.clone();
                tauri::async_runtime::spawn(async move {
                    // Helper to check if position is valid (within any monitor bounds)
                    let is_position_valid = |x: i32, y: i32, window: &tauri::WebviewWindow| -> bool {
                        if let Ok(monitors) = window.available_monitors() {
                            for monitor in monitors {
                                let pos = monitor.position();
                                let size = monitor.size();
                                // Check if position is within this monitor (with some margin)
                                if x >= pos.x - 100 && x < pos.x + size.width as i32 + 100
                                    && y >= pos.y - 100 && y < pos.y + size.height as i32 + 100
                                {
                                    return true;
                                }
                            }
                        }
                        false
                    };

                    // Helper to get bottom-right position
                    let get_bottom_right_position = |window: &tauri::WebviewWindow| -> Option<(i32, i32)> {
                        if let Ok(Some(monitor)) = window.current_monitor() {
                            let monitor_pos = monitor.position();
                            let monitor_size = monitor.size();
                            let window_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(300, 100));
                            let x = monitor_pos.x + monitor_size.width as i32 - window_size.width as i32 - 20;
                            let y = monitor_pos.y + monitor_size.height as i32 - window_size.height as i32 - 60;
                            Some((x, y))
                        } else {
                            None
                        }
                    };

                    // Try to load saved position and validate it
                    let use_saved = if let Ok(Some(pos)) = commands::load_window_position(app_handle_pos).await {
                        if is_position_valid(pos.x, pos.y, &window_clone) {
                            println!("Restoring window position: ({}, {})", pos.x, pos.y);
                            let _ = window_clone.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
                            true
                        } else {
                            println!("Saved position ({}, {}) is off-screen, using default", pos.x, pos.y);
                            false
                        }
                    } else {
                        false
                    };

                    // Use bottom-right corner as default if no valid saved position
                    if !use_saved {
                        if let Some((x, y)) = get_bottom_right_position(&window_clone) {
                            println!("Setting default window position: ({}, {})", x, y);
                            let _ = window_clone.set_position(tauri::PhysicalPosition::new(x, y));
                        }
                    }

                    // Show window after positioning
                    let _ = window_clone.show();
                });
            }

            let mut hotkey_manager = HotkeyManager::new(app.handle().clone())?;
            let registration = store::load_config(app.handle())
                .and_then(|config| parse_hotkey(&config.hotkey))
                .and_then(|(modifiers, key)| {
                    tauri::async_runtime::block_on(hotkey_manager.register(modifiers, key))
                });
            if let Err(error) = registration {
                log::error!("Failed to register initial recording shortcut: {}", error);
            }
            app.manage(Arc::new(Mutex::new(hotkey_manager)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::report_latency,
            commands::save_config_cmd,
            commands::start_recording,
            commands::stop_recording,
            commands::get_audio_level,
            commands::transcribe_and_insert,
            commands::open_config_window,
            commands::save_history_item,
            commands::load_history,
            commands::export_history,
            commands::clear_history,
            commands::update_stats,
            commands::get_stats,
            commands::save_window_position,
            commands::load_window_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
