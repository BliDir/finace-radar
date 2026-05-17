use anyhow::Result;
use serde_json::Value;

use crate::models::EmailAnalysis;

pub(super) fn analysis_json(text: &str, expected_id: &str) -> Result<EmailAnalysis> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let value: Value = serde_json::from_str(cleaned)?;
    let item = value
        .get("analyses")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(value);
    let mut analysis: EmailAnalysis = serde_json::from_value(item)?;
    analysis.id = expected_id.to_string();
    if analysis.confidence.trim().is_empty() {
        analysis.confidence = "medium".to_string();
    }
    Ok(analysis)
}
