use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub azure: AzureConfig,
    pub hotkey: HotkeyConfig,
    pub language: LanguageConfig,
    pub ui: UIConfig,
    pub features: FeatureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    pub speech_key: String,
    pub speech_region: String,
    pub openai_endpoint: String,
    pub openai_key: String,
    pub openai_deployment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConfig {
    pub modifier1: String,
    pub modifier2: Option<String>,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfig {
    pub speech_language: String,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIConfig {
    pub position_x: i32,
    pub position_y: i32,
    pub width: u32,
    pub height: u32,
    pub opacity: f64,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureConfig {
    pub text_polishing_enabled: bool,
    pub auto_insert_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            azure: AzureConfig {
                speech_key: String::new(),
                speech_region: "eastus".to_string(),
                openai_endpoint: String::new(),
                openai_key: String::new(),
                openai_deployment: "gpt-4".to_string(),
            },
            hotkey: HotkeyConfig {
                modifier1: "Ctrl".to_string(),
                modifier2: Some("Shift".to_string()),
                key: "Z".to_string(),
            },
            language: LanguageConfig {
                speech_language: "en-US".to_string(),
                model_version: "latest".to_string(),
            },
            ui: UIConfig {
                position_x: 0,
                position_y: 0,
                width: 300,
                height: 100,
                opacity: 0.9,
                theme: "light".to_string(),
            },
            features: FeatureConfig {
                text_polishing_enabled: true,
                auto_insert_enabled: true,
            },
        }
    }
}

pub mod store;
