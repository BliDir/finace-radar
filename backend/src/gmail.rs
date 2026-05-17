use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

use crate::models::EmailMessage;
use crate::utils::{clean_text, current_date, parse_month, postgres_safe_text, read_error};

#[derive(Deserialize)]
struct GmailListResponse {
    messages: Option<Vec<GmailRef>>,
}

#[derive(Deserialize)]
struct GmailRef {
    id: String,
}

#[derive(Deserialize)]
struct GmailMessageResponse {
    id: String,
    snippet: Option<String>,
    raw: Option<String>,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
}

pub(crate) async fn fetch_messages(
    http: &Client,
    token: &str,
    month: &str,
) -> Result<Vec<EmailMessage>> {
    let query = query_for_month(month)?;
    let list = http
        .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
        .bearer_auth(token)
        .query(&[("maxResults", "50"), ("q", query.as_str())])
        .send()
        .await
        .context("Gmail list request failed")?;
    if !list.status().is_success() {
        return Err(anyhow!("Gmail list failed: {}", read_error(list).await));
    }
    let refs = list
        .json::<GmailListResponse>()
        .await?
        .messages
        .unwrap_or_default();
    let mut messages = Vec::with_capacity(refs.len());
    for item in refs {
        let response = http
            .get(format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
                item.id
            ))
            .bearer_auth(token)
            .query(&[("format", "raw")])
            .send()
            .await
            .with_context(|| format!("Gmail message {} request failed", item.id))?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Gmail message fetch failed: {}",
                read_error(response).await
            ));
        }
        messages.push(normalize_message(
            response.json::<GmailMessageResponse>().await?,
        )?);
    }
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(messages)
}

fn query_for_month(month: &str) -> Result<String> {
    let (year, month_number) = parse_month(month)?;
    let start = NaiveDate::from_ymd_opt(year, month_number, 1).context("invalid month start")?;
    let (next_year, next_month) = if month_number == 12 {
        (year + 1, 1)
    } else {
        (year, month_number + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_year, next_month, 1).context("invalid month end")?;
    Ok(format!(
        "newer_than:365d (statement OR transaction OR transaksi OR notifikasi OR debit OR credit OR card OR bank OR payment OR charged OR receipt OR invoice OR transfer OR pembayaran OR merchant) after:{}/{}/{} before:{}/{}/{}",
        start.year(),
        start.month(),
        start.day(),
        end.year(),
        end.month(),
        end.day()
    ))
}

fn normalize_message(message: GmailMessageResponse) -> Result<EmailMessage> {
    let raw = decode_base64_url(&message.raw.unwrap_or_default())?;
    let headers = parse_raw_headers(&raw);
    let timestamp = message
        .internal_date
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let date = Utc
        .timestamp_millis_opt(timestamp)
        .single()
        .map(|value| value.date_naive().to_string())
        .unwrap_or_else(current_date);

    Ok(EmailMessage {
        id: postgres_safe_text(&message.id),
        from: postgres_safe_text(headers.get("from").map(String::as_str).unwrap_or("")),
        subject: postgres_safe_text(headers.get("subject").map(String::as_str).unwrap_or("")),
        date,
        snippet: postgres_safe_text(message.snippet.as_deref().unwrap_or("")),
        body: extract_raw_message_text(&raw),
        timestamp,
    })
}

fn decode_base64_url(value: &str) -> Result<String> {
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| general_purpose::URL_SAFE.decode(value))
        .or_else(|_| general_purpose::STANDARD.decode(value))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn parse_raw_headers(raw: &str) -> HashMap<String, String> {
    let header_text = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(h, _)| h)
        .unwrap_or("");
    let unfolded = Regex::new(r"\r?\n[ \t]+")
        .unwrap()
        .replace_all(header_text, " ");
    let mut headers = HashMap::new();
    for line in unfolded.lines() {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_lowercase(), decode_mime_words(value.trim()));
        }
    }
    headers
}

fn extract_raw_message_text(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n");
    let mut chunks = Vec::new();
    for part in normalized.split("\n--") {
        let (headers, body) = part.split_once("\n\n").unwrap_or(("", part));
        let lower_headers = headers.to_lowercase();
        if !lower_headers.contains("content-type: text/plain")
            && !lower_headers.contains("content-type: text/html")
            && chunks.len() > 8
        {
            continue;
        }
        let decoded = if lower_headers.contains("content-transfer-encoding: base64") {
            decode_base64_mime(body)
        } else if lower_headers.contains("content-transfer-encoding: quoted-printable") {
            decode_quoted_printable(body)
        } else {
            body.to_string()
        };
        let text = if lower_headers.contains("text/html") {
            html_to_text(&decoded)
        } else {
            decoded
        };
        let cleaned = clean_text(&text);
        if cleaned.len() > 20
            && !chunks
                .iter()
                .any(|existing: &String| existing.contains(&cleaned))
        {
            chunks.push(cleaned);
        }
    }
    clean_text(&chunks.join("\n\n"))
}

fn decode_base64_mime(value: &str) -> String {
    let compact = value.lines().map(str::trim).collect::<String>();
    general_purpose::STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| general_purpose::URL_SAFE.decode(compact.as_bytes()))
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_else(|_| value.to_string())
}

fn decode_quoted_printable(value: &str) -> String {
    let soft = value.replace("=\r\n", "").replace("=\n", "");
    let bytes = soft.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn html_to_text(value: &str) -> String {
    let mut text = Regex::new(r"(?i)<\s*(br|/tr|/p|/div|/li|/td|/th)[^>]*>")
        .unwrap()
        .replace_all(value, "\n")
        .to_string();
    text = Regex::new(r"(?is)<script[^>]*>.*?</script>")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();
    text = Regex::new(r"(?is)<style[^>]*>.*?</style>")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();
    text = Regex::new(r"(?s)<[^>]+>")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn decode_mime_words(value: &str) -> String {
    let decoded = Regex::new(r"=\?([^?]+)\?([BQbq])\?([^?]+)\?=")
        .unwrap()
        .replace_all(value, |caps: &regex::Captures| {
            let encoding = caps
                .get(2)
                .map(|m| m.as_str().to_ascii_uppercase())
                .unwrap_or_default();
            let data = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            if encoding == "B" {
                decode_base64_mime(data)
            } else {
                decode_quoted_printable(&data.replace('_', " "))
            }
        })
        .to_string();
    postgres_safe_text(&decoded)
}
