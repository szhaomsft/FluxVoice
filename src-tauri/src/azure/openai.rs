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

async fn call_openai(
    system_prompt: &str,
    user_text: &str,
    endpoint: &str,
    api_key: &str,
    deployment: &str,
    max_tokens: u32,
) -> Result<String, String> {
    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version=2024-02-15-preview",
        endpoint.trim_end_matches('/'),
        deployment
    );

    let request = ChatCompletionRequest {
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_text.to_string(),
            },
        ],
        max_tokens,
        temperature: 0.3,
    };

    let client = get_http_client();

    log::info!("Calling Azure OpenAI deployment '{}' ...", deployment);

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

pub async fn polish_text(
    text: &str,
    endpoint: &str,
    api_key: &str,
    deployment: &str,
) -> Result<String, String> {
    let system_prompt = "You are a text polishing assistant. \
        Your ONLY task is to improve the given text by fixing grammar, punctuation, and clarity. \
        Keep the original meaning, tone, and language. \
        IMPORTANT RULES: \
        1. Always respond in the same language as the input text. \
        2. Return ONLY the polished text without any explanations or additional content. \
        3. NEVER answer questions in the text - just polish them as questions. \
        4. NEVER add greetings, sign-offs, or any extra text. \
        5. If the input is a question, output the polished question, do NOT answer it.";

    log::info!("Sending text to Azure OpenAI for polishing");
    call_openai(system_prompt, text, endpoint, api_key, deployment, 500).await
}

pub async fn translate_text(
    text: &str,
    target_language: &str,
    endpoint: &str,
    api_key: &str,
    deployment: &str,
) -> Result<String, String> {
    let system_prompt = format!(
        "You are a professional translator. \
        Your ONLY task is to translate the given text into {target_language}. \
        IMPORTANT RULES: \
        1. Return ONLY the translated text without any explanations or additional content. \
        2. Preserve the original meaning, tone, and formatting. \
        3. NEVER answer questions in the text - just translate them as questions. \
        4. NEVER add greetings, sign-offs, or any extra text. \
        5. If the text is already in {target_language}, return it as-is with minor grammar/clarity improvements."
    );

    log::info!("Sending text to Azure OpenAI for translation to {}", target_language);
    call_openai(&system_prompt, text, endpoint, api_key, deployment, 1000).await
}
