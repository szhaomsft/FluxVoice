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
    pub multilingual: bool,  // When true, send empty locales to use multi-lingual model
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
    #[serde(default = "default_post_processing_mode")]
    pub post_processing_mode: String,  // "none", "polish", or "translate"
    #[serde(default = "default_translate_target_language")]
    pub translate_target_language: String,  // e.g. "English", "Japanese"
    pub auto_insert_enabled: bool,
    // Keep old field for backwards compatibility (will be migrated on save)
    #[serde(skip_serializing, default)]
    text_polishing_enabled: Option<bool>,
}

fn default_post_processing_mode() -> String {
    "none".to_string()
}

fn default_translate_target_language() -> String {
    "English".to_string()
}

impl FeatureConfig {
    pub fn migrate(&mut self) {
        // Migrate old text_polishing_enabled to post_processing_mode if needed
        if let Some(old_polish) = self.text_polishing_enabled.take() {
            if self.post_processing_mode == "none" && old_polish {
                self.post_processing_mode = "polish".to_string();
            }
        }
    }
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
                multilingual: false,
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
                post_processing_mode: "none".to_string(),
                translate_target_language: "English".to_string(),
                auto_insert_enabled: true,
                text_polishing_enabled: None,
            },
        }
    }
}

pub mod store;
