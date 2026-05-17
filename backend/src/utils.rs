use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, Utc};
use regex::Regex;

pub(crate) async fn read_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if text.is_empty() {
        status.to_string()
    } else {
        format!("{status}: {text}")
    }
}

/// PostgreSQL rejects NUL bytes in UTF-8 text; email MIME decoding can produce them.
pub(crate) fn postgres_safe_text(value: &str) -> String {
    value.replace('\0', "")
}

pub(crate) fn postgres_safe_opt(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|item| postgres_safe_text(item))
}

pub(crate) fn clean_text(value: &str) -> String {
    let value = postgres_safe_text(value);
    Regex::new(r"[ \t]+")
        .unwrap()
        .replace_all(&value, " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn extract_email_address(value: &str) -> String {
    Regex::new(r"<([^>]+)>")
        .unwrap()
        .captures(value)
        .and_then(|caps| caps.get(1).map(|item| item.as_str().to_string()))
        .unwrap_or_else(|| value.trim().to_string())
}

pub(crate) fn parse_month(month: &str) -> Result<(i32, u32)> {
    let (year, month) = month.split_once('-').context("month must be YYYY-MM")?;
    Ok((year.parse()?, month.parse()?))
}

pub(crate) fn month_range_bounds(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate)> {
    let (from_year, from_month) = parse_month(from)?;
    let (to_year, to_month) = parse_month(to)?;
    let start = NaiveDate::from_ymd_opt(from_year, from_month, 1).context("invalid from month")?;
    let (next_year, next_month) = if to_month == 12 {
        (to_year + 1, 1)
    } else {
        (to_year, to_month + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_year, next_month, 1).context("invalid to month")?;
    Ok((start, end))
}

pub(crate) fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub(crate) fn current_month() -> String {
    let now = Utc::now().date_naive();
    format!("{:04}-{:02}", now.year(), now.month())
}

pub(crate) fn current_date() -> String {
    Utc::now().date_naive().to_string()
}
