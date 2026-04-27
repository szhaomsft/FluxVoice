export interface AppConfig {
  azure: AzureConfig;
  hotkey: HotkeyConfig;
  language: LanguageConfig;
  ui: UIConfig;
  features: FeatureConfig;
}

export interface AzureConfig {
  speechKey: string;
  speechRegion: string;
  openaiEndpoint: string;
  openaiKey: string;
  openaiDeployment: string;
}

export interface HotkeyConfig {
  modifier1: string;
  modifier2?: string;
  key: string;
}

export interface LanguageConfig {
  speechLanguages: string[];  // Changed from speechLanguage to support multiple languages
  multilingual: boolean;      // When true, use multi-lingual model (auto-detects languages)
  modelVersion: string;
}

export interface UIConfig {
  positionX: number;
  positionY: number;
  width: number;
  height: number;
  opacity: number;
  theme: 'light' | 'dark';
}

export interface FeatureConfig {
  postProcessingMode: 'none' | 'polish' | 'translate';
  translateTargetLanguage: string;  // e.g. "English", "Japanese"
  autoInsertEnabled: boolean;
}

export type RecordingState = 'idle' | 'recording' | 'processing' | 'error';
