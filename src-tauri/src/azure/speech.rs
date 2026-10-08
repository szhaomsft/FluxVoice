use serde::{Deserialize, Serialize};
use reqwest::multipart;
use super::get_http_client;
use crate::config::{LanguageConfig, SttModel};

#[derive(Debug, Deserialize)]
struct FastTranscriptionResponse {
    #[serde(rename = "combinedPhrases")]
    combined_phrases: Option<Vec<CombinedPhrase>>,
    phrases: Option<Vec<Phrase>>,
}

#[derive(Debug, Deserialize)]
struct CombinedPhrase {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Phrase {
    text: Option<String>,
    #[allow(dead_code)]
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct TranscriptionDefinition {
    locales: Vec<String>,
    #[serde(rename = "phraseList", skip_serializing_if = "Option::is_none")]
    phrase_list: Option<PhraseList>,
    #[serde(rename = "enhancedMode", skip_serializing_if = "Option::is_none")]
    enhanced_mode: Option<EnhancedMode>,
}

#[derive(Debug, Serialize)]
struct PhraseList {
    phrases: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EnhancedMode {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'static str>,
}

fn build_definition(language: &LanguageConfig, phrases: &[String]) -> Result<TranscriptionDefinition, String> {
    let mut locales = if language.multilingual {
        Vec::new()
    } else {
        language.speech_languages.clone()
    };
    if language.stt_model == SttModel::MaiTranscribe2 {
        locales = locales.iter().map(|locale| {
            if locale.eq_ignore_ascii_case("zh-HK") {
                "yue".to_string()
            } else {
                locale.split('-').next().unwrap_or(locale).to_lowercase()
            }
        }).collect();
        locales.sort();
        locales.dedup();
        if locales.len() > 1 {
            return Err("MAI-Transcribe-2 accepts only one language hint. Select one language or enable Multilingual for automatic detection.".to_string());
        }
    }
    let enhanced_mode = match language.stt_model {
        SttModel::Fast => None,
        SttModel::LlmSpeech => Some(EnhancedMode {
            enabled: true,
            task: Some("transcribe"),
            model: None,
        }),
        SttModel::MaiTranscribe2 => Some(EnhancedMode {
            enabled: true,
            task: None,
            model: Some("MAI-Transcribe-2"),
        }),
    };
    let phrase_list = if phrases.is_empty() {
        None
    } else {
        Some(PhraseList { phrases: phrases.to_vec() })
    };
    Ok(TranscriptionDefinition { locales, enhanced_mode, phrase_list })
}

pub fn validate_model_settings(language: &LanguageConfig) -> Result<(), String> {
    build_definition(language, &[]).map(|_| ())
}

fn audio_content_type(audio_data: &[u8], model: SttModel) -> Result<(&'static str, &'static str), String> {
    if audio_data.starts_with(b"OggS") {
        if model == SttModel::MaiTranscribe2 {
            return Err("MAI-Transcribe-2 requires MP3 or WAV audio. Start a new recording with this model selected.".to_string());
        }
        Ok(("audio.ogg", "audio/ogg"))
    } else if audio_data.starts_with(b"RIFF") && audio_data.get(8..12) == Some(b"WAVE") {
        Ok(("audio.wav", "audio/wav"))
    } else if (audio_data.starts_with(b"ID3") && audio_data.len() >= 10)
        || audio_data.get(..4).is_some_and(|header| {
            header[0] == 0xff && header[1] & 0xe0 == 0xe0
                && header[1] & 0x06 == 0x02 && header[1] & 0x18 != 0x08
                && header[2] & 0xf0 != 0 && header[2] & 0xf0 != 0xf0
                && header[2] & 0x0c != 0x0c
        })
    {
        Ok(("audio.mp3", "audio/mpeg"))
    } else {
        Err("Unsupported recording format: expected Opus/OGG, MP3, or WAV audio.".to_string())
    }
}

pub async fn transcribe_audio(
    audio_data: Vec<u8>,
    subscription_key: &str,
    region: &str,
    language: &LanguageConfig,
    phrases: &[String],
) -> Result<String, String> {
    // Use Fast Transcription API with multi-language support
    let url = format!(
        "https://{}.api.cognitive.microsoft.com/speechtotext/transcriptions:transcribe?api-version=2025-10-15",
        region
    );

    let client = get_http_client();

    let definition = build_definition(language, phrases)?;
    let (file_name, mime_type) = audio_content_type(&audio_data, language.stt_model)?;
    println!("[latency] speech_model={:?} locales={:?} audio_format={}", language.stt_model, definition.locales, mime_type);

    let definition_json = serde_json::to_string(&definition)
        .map_err(|e| format!("Failed to serialize definition: {}", e))?;

    // Match the multipart metadata to the actual recording format.
    let audio_part = multipart::Part::bytes(audio_data)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|e| format!("Failed to create audio part: {}", e))?;

    let definition_part = multipart::Part::text(definition_json)
        .mime_str("application/json")
        .map_err(|e| format!("Failed to create definition part: {}", e))?;

    let form = multipart::Form::new()
        .part("audio", audio_part)
        .part("definition", definition_part);

    let request_started = std::time::Instant::now();
    let response_result = client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", subscription_key)
        .multipart(form)
        .send()
        .await;
    println!("[latency] speech_request_to_headers_ms={:.1} success={}", request_started.elapsed().as_secs_f64() * 1000.0, response_result.is_ok());
    let response = response_result.map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    println!("[latency] speech_http_status={}", status.as_u16());

    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_body));
    }

    let body_started = std::time::Instant::now();
    let result: FastTranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    println!("[latency] speech_response_body_parse_ms={:.1}", body_started.elapsed().as_secs_f64() * 1000.0);

    // Extract text from combinedPhrases (preferred) or phrases
    if let Some(combined) = result.combined_phrases {
        if let Some(first) = combined.first() {
            if let Some(text) = &first.text {
                if !text.is_empty() {
                    log::info!("Transcription successful");
                    return Ok(text.clone());
                }
            }
        }
    }

    // Fallback to concatenating phrases
    if let Some(phrases) = result.phrases {
        let text: String = phrases
            .iter()
            .filter_map(|p| p.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");

        if !text.is_empty() {
            log::info!("Transcription successful (from phrases)");
            return Ok(text);
        }
    }

    Err("No transcription text in response".to_string())
}

pub async fn transcribe_audio_with_retry(
    audio_data: Vec<u8>,
    subscription_key: &str,
    region: &str,
    language: &LanguageConfig,
    phrases: &[String],
    max_retries: u32,
) -> Result<String, String> {
    validate_model_settings(language)?;
    audio_content_type(&audio_data, language.stt_model)?;
    for attempt in 0..max_retries {
        println!("[latency] speech_attempt={}", attempt + 1);
        match transcribe_audio(
            audio_data.clone(),
            subscription_key,
            region,
            language,
            phrases,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                log::warn!("Transcription attempt {} failed: {}. Retrying...", attempt + 1, e);
                println!("[latency] speech_retry_backoff_ms={}", 1000 * 2_u64.pow(attempt));
                tokio::time::sleep(tokio::time::Duration::from_secs(2_u64.pow(attempt))).await;
            }
            Err(e) => {
                return Err(format!(
                    "Transcription failed after {} attempts: {}",
                    max_retries, e
                ))
            }
        }
    }
    Err("Unexpected error in retry logic".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use serde_json::json;

    fn language(model: SttModel, multilingual: bool, locales: &[&str]) -> LanguageConfig {
        let mut language = AppConfig::default().language;
        language.stt_model = model;
        language.multilingual = multilingual;
        language.speech_languages = locales.iter().map(|locale| locale.to_string()).collect();
        language
    }

    #[test]
    fn request_definitions_select_each_model_explicitly() {
        for (model, enhanced) in [
            (SttModel::Fast, None),
            (SttModel::LlmSpeech, Some(json!({"enabled": true, "task": "transcribe"}))),
            (SttModel::MaiTranscribe2, Some(json!({"enabled": true, "model": "MAI-Transcribe-2"}))),
        ] {
            let definition = build_definition(&language(model, true, &["en-US", "zh-CN"]), &[]).unwrap();
            let actual = serde_json::to_value(definition).unwrap();
            let expected = match enhanced {
                Some(enhanced) => json!({"locales": [], "enhancedMode": enhanced}),
                None => json!({"locales": []}),
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn fast_and_llm_preserve_configured_locale_candidates() {
        for model in [SttModel::Fast, SttModel::LlmSpeech] {
            let definition = build_definition(&language(model, false, &["en-US", "zh-CN"]), &[]).unwrap();
            assert_eq!(definition.locales, vec!["en-US", "zh-CN"]);
        }
    }

    #[test]
    fn mai_normalizes_a_single_language_hint() {
        for (locales, expected) in [
            (vec!["en-US", "en-GB"], "en"),
            (vec!["zh-CN"], "zh"),
            (vec!["zh-HK"], "yue"),
        ] {
            let definition = build_definition(&language(SttModel::MaiTranscribe2, false, &locales), &[]).unwrap();
            assert_eq!(definition.locales, vec![expected]);
        }
    }

    #[test]
    fn mai_rejects_multiple_language_hints() {
        let result = build_definition(&language(SttModel::MaiTranscribe2, false, &["en-US", "zh-CN"]), &[]);
        assert!(result.unwrap_err().contains("enable Multilingual"));
    }

    #[test]
    fn multipart_audio_metadata_matches_format_and_model() {
        assert_eq!(audio_content_type(b"OggS", SttModel::Fast).unwrap(), ("audio.ogg", "audio/ogg"));
        assert_eq!(audio_content_type(b"OggS", SttModel::LlmSpeech).unwrap(), ("audio.ogg", "audio/ogg"));
        assert_eq!(audio_content_type(b"RIFF\x00\x00\x00\x00WAVE", SttModel::MaiTranscribe2).unwrap(), ("audio.wav", "audio/wav"));
        assert_eq!(audio_content_type(&[0xff, 0xf3, 0x68, 0xc4], SttModel::MaiTranscribe2).unwrap(), ("audio.mp3", "audio/mpeg"));
        assert_eq!(audio_content_type(b"ID3\x04\x00\x00\x00\x00\x00\x00", SttModel::MaiTranscribe2).unwrap(), ("audio.mp3", "audio/mpeg"));
        assert!(audio_content_type(b"OggS", SttModel::MaiTranscribe2).is_err());
        assert!(audio_content_type(b"ID3", SttModel::MaiTranscribe2).is_err());
        assert!(audio_content_type(&[0xff, 0xff, 0xff, 0xff], SttModel::MaiTranscribe2).is_err());
        assert!(audio_content_type(b"RIFF", SttModel::MaiTranscribe2).is_err());
        assert!(audio_content_type(b"invalid", SttModel::Fast).is_err());
    }
    #[test]
    fn hints_preserve_model_selection_and_normalized_locales() {
        for model in [SttModel::Fast, SttModel::LlmSpeech, SttModel::MaiTranscribe2] {
            for multilingual in [false, true] {
                let language = language(model, multilingual, &["en-US"]);
                let baseline = serde_json::to_value(build_definition(&language, &[]).unwrap()).unwrap();
                assert!(baseline.get("phraseList").is_none());
                let actual = serde_json::to_value(
                    build_definition(&language, &["FluxVoice".into(), "Rehaan".into()]).unwrap()
                ).unwrap();
                let mut expected = baseline;
                expected["phraseList"] = json!({"phrases": ["FluxVoice", "Rehaan"]});
                assert_eq!(actual, expected);
            }
        }
    }
}
