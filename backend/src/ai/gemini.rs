use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{EmailAnalysis, EmailMessage};

use super::{parser, prompt, read_error, AiConfig};

pub(super) async fn analyze(
    http: &Client,
    config: &AiConfig,
    message: &EmailMessage,
) -> Result<EmailAnalysis> {
    if config.gemini_key.is_empty() {
        return Err(anyhow!("GEMINI_API_KEY is not set"));
    }
    let response = http
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            config.gemini_model
        ))
        .header("x-goog-api-key", &config.gemini_key)
        .json(&json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt::for_email(message) }] }],
            "generationConfig": { "responseMimeType": "application/json", "temperature": 0.0 }
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!("{}", read_error(response).await));
    }
    let value: Value = response.json().await?;
    let text = value["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow!("Gemini returned no text"))?;
    parser::analysis_json(text, &message.id)
}
