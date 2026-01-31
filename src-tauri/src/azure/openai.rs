use serde::{Deserialize, Serialize};
use super::get_http_client;

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
}

pub async fn polish_text(
    text: &str,
    endpoint: &str,
    api_key: &str,
    deployment: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
        endpoint.trim_end_matches('/'),
        deployment
    );

    let system_message = "You are a text polishing assistant. \
        Improve the given text by fixing grammar, punctuation, and clarity. \
        Keep the original meaning and tone. Return only the polished text without explanations.";

    let request = ChatCompletionRequest {
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_message.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: text.to_string(),
            },
        ],
        max_tokens: 500,
        temperature: 0.3,
    };

    let client = get_http_client();

    log::info!("Sending text to Azure OpenAI for polishing");

    let response = client
        .post(&url)
        .header("api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error ({}): {}", status, error_body));
    }

    let result: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    result
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .ok_or_else(|| "No response from OpenAI".to_string())
}
