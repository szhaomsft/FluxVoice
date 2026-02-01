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
    #[serde(default = "default_speech_languages")]
    pub speech_languages: Vec<String>,  // Changed from speech_language to support multiple languages
    #[serde(default)]
    pub model_version: String,
    // Keep old field for backwards compatibility (will be migrated on save)
    #[serde(skip_serializing, default)]
    speech_language: Option<String>,
}

fn default_speech_languages() -> Vec<String> {
    vec!["en-US".to_string()]
}

impl LanguageConfig {
    pub fn migrate(&mut self) {
        // Migrate old speech_language to speech_languages if needed
        if self.speech_languages.is_empty() {
            if let Some(old_lang) = self.speech_language.take() {
                self.speech_languages = vec![old_lang];
            } else {
                self.speech_languages = default_speech_languages();
            }
        }
    }
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
                speech_languages: vec!["en-US".to_string()],
                model_version: "latest".to_string(),
                speech_language: None,
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
