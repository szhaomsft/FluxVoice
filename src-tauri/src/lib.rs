mod audio;
mod azure;
mod config;
mod commands;
mod hotkey;
mod input;

use crate::audio::AudioRecorder;
use crate::commands::AppState;
use crate::config::store;
use crate::hotkey::{parse_key, parse_modifier, HotkeyManager};
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
                    .expect("Failed to initialize audio recorder"),
            ));
            let injector = Arc::new(Mutex::new(TextInjector::new()));

            app.manage(AppState { recorder, injector });

            // Position main window
            if let Some(window) = app.get_webview_window("main") {
                let app_handle_pos = app.handle().clone();
                let window_clone = window.clone();
                tauri::async_runtime::spawn(async move {
                    // Try to load saved position
                    if let Ok(Some(pos)) = commands::load_window_position(app_handle_pos).await {
                        println!("Restoring window position: ({}, {})", pos.x, pos.y);
                        let _ = window_clone.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
                    } else {
                        // Default to bottom-right corner
                        if let Ok(Some(monitor)) = window_clone.current_monitor() {
                            let monitor_size = monitor.size();
                            let window_size = window_clone.outer_size().unwrap_or(tauri::PhysicalSize::new(300, 100));
                            let x = monitor_size.width as i32 - window_size.width as i32 - 20;
                            let y = monitor_size.height as i32 - window_size.height as i32 - 60;
                            println!("Setting default window position: ({}, {})", x, y);
                            let _ = window_clone.set_position(tauri::PhysicalPosition::new(x, y));
                        }
                    }
                    // Show window after positioning
                    let _ = window_clone.show();
                });
            }

            // Register initial hotkey
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match store::load_config(&app_handle) {
                    Ok(config) => {
                        println!("Config loaded: hotkey = {} + {:?} + {}",
                            config.hotkey.modifier1,
                            config.hotkey.modifier2,
                            config.hotkey.key);

                        // Parse and register hotkey
                        if let Some(modifier1) = parse_modifier(&config.hotkey.modifier1) {
                            let mut modifiers = modifier1;

                            if let Some(ref modifier2_str) = config.hotkey.modifier2 {
                                if let Some(modifier2) = parse_modifier(modifier2_str) {
                                    modifiers |= modifier2;
                                }
                            }

                            if let Some(key) = parse_key(&config.hotkey.key) {
                                let mut hotkey_manager = HotkeyManager::new(app_handle.clone())
                                    .expect("Failed to create hotkey manager");

                                if let Err(e) = hotkey_manager.register(modifiers, key).await {
                                    println!("ERROR: Failed to register hotkey: {}", e);
                                } else {
                                    println!(
                                        "SUCCESS: Hotkey registered: {:?} + {}",
                                        modifiers,
                                        config.hotkey.key
                                    );
                                }

                                // Keep hotkey manager alive
                                app_handle.manage(Arc::new(Mutex::new(hotkey_manager)));
                            } else {
                                println!("ERROR: Failed to parse key: {}", config.hotkey.key);
                            }
                        } else {
                            println!("ERROR: Failed to parse modifier1: {}", config.hotkey.modifier1);
                        }
                    }
                    Err(e) => {
                        println!("ERROR: Failed to load config: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config_cmd,
            commands::start_recording,
            commands::stop_recording,
            commands::get_audio_level,
            commands::transcribe_and_insert,
            commands::open_config_window,
            commands::save_history_item,
            commands::load_history,
            commands::clear_history,
            commands::update_stats,
            commands::get_stats,
            commands::save_window_position,
            commands::load_window_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

