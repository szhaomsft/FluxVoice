use serde::{Deserialize, Serialize};
use reqwest::multipart;
use super::get_http_client;

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
    languages: &[String],  // Changed to support multiple languages
    multilingual: bool,     // When true, send empty locales for multi-lingual model
) -> Result<String, String> {
    // Use Fast Transcription API with multi-language support
    let url = format!(
        "https://{}.api.cognitive.microsoft.com/speechtotext/transcriptions:transcribe?api-version=2025-10-15",
        region
    );

    let client = get_http_client();

    // In multilingual mode, send empty locales to let the API auto-detect
    let locales = if multilingual {
        log::info!("Sending {} bytes of Opus audio to Azure Fast Transcription API (multilingual mode)", audio_data.len());
        println!(">>> Transcribing in multilingual mode");
        vec![]
    } else {
        log::info!("Sending {} bytes of Opus audio to Azure Fast Transcription API (languages: {:?})", audio_data.len(), languages);
        println!(">>> Transcribing with languages: {:?}", languages);
        languages.to_vec()
    };

    // Build definition with configured locales for auto-detection
    let definition = TranscriptionDefinition {
        locales,
    };

    let definition_json = serde_json::to_string(&definition)
        .map_err(|e| format!("Failed to serialize definition: {}", e))?;

    // Create multipart form with Opus/OGG audio
    let audio_part = multipart::Part::bytes(audio_data)
        .file_name("audio.ogg")
        .mime_str("audio/ogg")
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
    languages: &[String],  // Changed to support multiple languages
    multilingual: bool,     // When true, send empty locales for multi-lingual model
    max_retries: u32,
) -> Result<String, String> {
    for attempt in 0..max_retries {
        println!("[latency] speech_attempt={}", attempt + 1);
        match transcribe_audio(
            audio_data.clone(),
            subscription_key,
            region,
            languages,
            multilingual,
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
