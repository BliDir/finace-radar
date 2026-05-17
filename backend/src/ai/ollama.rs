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
    let response = http
        .post(format!(
            "{}/api/chat",
            config.ollama_base_url.trim_end_matches('/')
        ))
        .json(&json!({
            "model": config.ollama_model,
            "stream": false,
            "format": "json",
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
    let text = value["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("Ollama returned no content"))?;
    parser::analysis_json(text, &message.id)
}
