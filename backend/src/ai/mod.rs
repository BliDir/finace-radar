mod gemini;
mod groq;
mod ollama;
mod parser;
mod prompt;

use std::env;

use anyhow::{anyhow, Result};
use reqwest::Client;

use crate::models::{EmailAnalysis, EmailMessage};

#[derive(Clone)]
pub(crate) struct AiConfig {
    pub(crate) providers: Vec<String>,
    pub(crate) gemini_key: String,
    pub(crate) gemini_model: String,
    pub(crate) groq_key: String,
    pub(crate) groq_model: String,
    pub(crate) ollama_base_url: String,
    pub(crate) ollama_model: String,
}

impl AiConfig {
    pub(crate) fn from_env() -> Self {
        let providers = env::var("AI_PROVIDERS")
            .unwrap_or_else(|_| "gemini,groq,ollama".to_string())
            .split(',')
            .map(|item| item.trim().to_lowercase())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();

        Self {
            providers,
            gemini_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            gemini_model: env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash-lite".to_string()),
            groq_key: env::var("GROQ_API_KEY").unwrap_or_default(),
            groq_model: env::var("GROQ_MODEL")
                .unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string()),
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            ollama_model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string()),
        }
    }

    pub(crate) fn primary(&self) -> (&str, &str) {
        match self
            .providers
            .first()
            .map(String::as_str)
            .unwrap_or("gemini")
        {
            "groq" => ("groq", &self.groq_model),
            "ollama" => ("ollama", &self.ollama_model),
            _ => ("gemini", &self.gemini_model),
        }
    }

    pub(crate) fn model_for_provider(&self, provider: &str) -> &str {
        match provider {
            "groq" => &self.groq_model,
            "ollama" => &self.ollama_model,
            _ => &self.gemini_model,
        }
    }
}

pub(crate) async fn analyze(
    http: &Client,
    config: &AiConfig,
    provider: &str,
    message: &EmailMessage,
) -> Result<EmailAnalysis> {
    match provider {
        "gemini" => gemini::analyze(http, config, message).await,
        "groq" => groq::analyze(http, config, message).await,
        "ollama" => ollama::analyze(http, config, message).await,
        other => Err(anyhow!("unknown AI provider {other}")),
    }
}

async fn read_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if text.is_empty() {
        status.to_string()
    } else {
        format!("{status}: {text}")
    }
}
