use super::AppConfig;
use tauri_plugin_store::StoreExt;

const CONFIG_STORE_FILE: &str = "config.json";

pub fn load_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let store = app
        .store(CONFIG_STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let config_value = store.get("app_config");

    match config_value {
        Some(value) => {
            let mut config: AppConfig = serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to deserialize config: {}", e))?;

            // Migrate old config fields to new format
            config.language.migrate();

            Ok(config)
        }
        None => {
            // Return default config if not found
            let default_config = AppConfig::default();
            save_config(app, &default_config)?;
            Ok(default_config)
        }
    }
}

pub fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let store = app
        .store(CONFIG_STORE_FILE)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let config_value = serde_json::to_value(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    store.set("app_config", config_value);

    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}
