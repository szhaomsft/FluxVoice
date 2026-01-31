use serde::{Deserialize, Serialize};
use reqwest::multipart;

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
}

pub async fn transcribe_audio(
    audio_data: Vec<u8>,
    subscription_key: &str,
    region: &str,
    _language: &str, // Kept for API compatibility, but we now use auto-detection
) -> Result<String, String> {
    // Use Fast Transcription API with multi-language support
    let url = format!(
        "https://{}.api.cognitive.microsoft.com/speechtotext/transcriptions:transcribe?api-version=2024-11-15",
        region
    );

    let client = reqwest::Client::new();

    log::info!("Sending {} bytes of audio to Azure Fast Transcription API (en-US, zh-CN)", audio_data.len());

    // Build definition with multiple locales for auto-detection
    let definition = TranscriptionDefinition {
        locales: vec!["en-US".to_string(), "zh-CN".to_string()],
    };

    let definition_json = serde_json::to_string(&definition)
        .map_err(|e| format!("Failed to serialize definition: {}", e))?;

    // Create multipart form
    let audio_part = multipart::Part::bytes(audio_data)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Failed to create audio part: {}", e))?;

    let definition_part = multipart::Part::text(definition_json)
        .mime_str("application/json")
        .map_err(|e| format!("Failed to create definition part: {}", e))?;

    let form = multipart::Form::new()
        .part("audio", audio_part)
        .part("definition", definition_part);

    let response = client
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", subscription_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_body));
    }

    let result: FastTranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

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
    language: &str,
    max_retries: u32,
) -> Result<String, String> {
    for attempt in 0..max_retries {
        match transcribe_audio(
            audio_data.clone(),
            subscription_key,
            region,
            language,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                log::warn!("Transcription attempt {} failed: {}. Retrying...", attempt + 1, e);
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
