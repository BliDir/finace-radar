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
    if config.groq_key.is_empty() {
        return Err(anyhow!("GROQ_API_KEY is not set"));
    }
    let response = http
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(&config.groq_key)
        .json(&json!({
            "model": config.groq_model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "Return strict JSON only." },
                { "role": "user", "content": prompt::for_email(message) }
            ]
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!("{}", read_error(response).await));
    }
    let value: Value = response.json().await?;
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("Groq returned no content"))?;
    parser::analysis_json(text, &message.id)
}
