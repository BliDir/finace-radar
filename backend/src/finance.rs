use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use tracing::info;

use crate::ai::{self, AiConfig};
use crate::models::{EmailAnalysis, EmailMessage};
use crate::utils::extract_email_address;

pub(crate) async fn analyze_messages(
    http: &Client,
    ai_config: &AiConfig,
    messages: &[EmailMessage],
) -> Result<Vec<EmailAnalysis>> {
    if messages.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::with_capacity(messages.len());
    let mut provider_used = String::new();
    let mut model_used = String::new();

    for message in messages {
        if let Some(reason) = non_finance_reason(message) {
            results.push(non_finance(message, reason));
            continue;
        }

        let mut errors = Vec::new();
        let mut analysis = None;
        for provider in &ai_config.providers {
            let attempt = ai::analyze(http, ai_config, provider, message).await;

            match attempt {
                Ok(mut item) => {
                    provider_used = provider.clone();
                    model_used = ai_config.model_for_provider(provider).to_string();
                    repair_analysis(message, &mut item);
                    analysis = Some(item);
                    break;
                }
                Err(error) => {
                    let text = error.to_string();
                    errors.push(format!("{provider}: {text}"));
                    if !is_retryable_ai_error(&text) {
                        break;
                    }
                }
            }
        }

        results.push(analysis.unwrap_or_else(|| non_finance(message, errors.join(" | "))));
    }

    if !provider_used.is_empty() {
        info!("AI analysis completed with {provider_used} {model_used}");
    }
    Ok(results)
}

fn repair_analysis(message: &EmailMessage, analysis: &mut EmailAnalysis) {
    if let Some(reason) = non_finance_reason(message) {
        *analysis = non_finance(message, reason);
        return;
    }

    let text = format!("{} {} {}", message.subject, message.snippet, message.body);
    let lower = text.to_lowercase();

    if is_marketing(&lower) && !has_transaction_signal(&lower) {
        *analysis = non_finance(message, "marketing or promotion".to_string());
        return;
    }

    if analysis.amount.is_none() {
        if let Some((amount, currency)) = extract_amount(&text) {
            if !is_promotional_amount_context(&lower) {
                analysis.amount = Some(amount);
                analysis.currency.get_or_insert(currency);
                if analysis.is_finance && analysis.confidence == "low" {
                    analysis.confidence = "medium".to_string();
                }
            }
        }
    } else if is_promotional_amount_context(&lower) && !has_transaction_signal(&lower) {
        analysis.amount = None;
        analysis.currency = None;
    }

    if has_transaction_signal(&lower) {
        analysis.is_finance = true;
        if analysis.direction == "non_finance" {
            analysis.direction = "spending".to_string();
        }
        if analysis.amount.is_none() {
            if let Some((amount, currency)) = extract_amount(&text) {
                if !is_promotional_amount_context(&lower) {
                    analysis.amount = Some(amount);
                    analysis.currency.get_or_insert(currency);
                }
            }
        }
    } else if !(analysis.is_finance && analysis.amount.is_some()) {
        analysis.is_finance = false;
        analysis.direction = "non_finance".to_string();
        analysis.amount = None;
        analysis.currency = None;
    }
    if analysis.is_finance && analysis.date.is_none() {
        analysis.date = extract_date(&text).or_else(|| Some(message.date.clone()));
    }
    if analysis.is_finance && analysis.currency.is_none() {
        analysis.currency = infer_currency(&text, &message.from);
    }
    if analysis.is_finance && analysis.merchant.is_none() {
        analysis.merchant = merchant_from_subject(&message.subject)
            .or_else(|| Some(merchant_from_sender(&message.from, &message.subject)));
    }
    if analysis.is_finance && analysis.account.is_none() {
        analysis.account = Some(account_from_text(&text, &message.from));
    }
}

fn non_finance(message: &EmailMessage, reason: String) -> EmailAnalysis {
    EmailAnalysis {
        id: message.id.clone(),
        is_finance: false,
        direction: "non_finance".to_string(),
        amount: None,
        currency: None,
        date: Some(message.date.clone()),
        from: None,
        to: None,
        account: None,
        account_type: None,
        merchant: None,
        category: Some(reason),
        confidence: "low".to_string(),
    }
}

fn extract_amount(text: &str) -> Option<(f64, String)> {
    let amount_pattern = r"[0-9][0-9.,\s]*[0-9]|[0-9]";
    let unit_pattern = r"(?P<unit>rb|ribu|jt|juta|thousand|million|k|m)?";
    let currency_pattern =
        r"(?P<currency>IDR|JPY|USD|EUR|SGD|AUD|GBP|Rp\.?|rupiah|¥|円|yen|US\$|\$|dollars?)";

    let glued = Regex::new(&format!(
        r"(?i){currency_pattern}(?P<amount>[0-9][0-9.,]+){unit_pattern}"
    ))
    .unwrap();
    for caps in glued.captures_iter(text) {
        let currency = normalize_currency(caps.name("currency")?.as_str());
        let amount = parse_amount_with_unit(
            caps.name("amount")?.as_str(),
            &currency,
            caps.name("unit").map(|item| item.as_str()),
        )?;
        if amount > 0.0 {
            return Some((amount, currency));
        }
    }

    let currency_first = Regex::new(&format!(
        r"(?ix)
        (?:
          (?:amount|total|charged|paid|payment|debit|credit|transfer\s+amount|transaction\s+amount|sejumlah|nilai|nominal|jumlah|tagihan)\s*[:\-]?\s*
        )?
        {currency_pattern}\s*
        (?P<amount>{amount_pattern})\s*
        {unit_pattern}
        "
    ))
    .unwrap();
    for caps in currency_first.captures_iter(text) {
        let currency = normalize_currency(caps.name("currency")?.as_str());
        let amount = parse_amount_with_unit(
            caps.name("amount")?.as_str(),
            &currency,
            caps.name("unit").map(|item| item.as_str()),
        )?;
        if amount > 0.0 {
            return Some((amount, currency));
        }
    }

    let amount_first = Regex::new(&format!(
        r"(?ix)
        (?P<amount>{amount_pattern})\s*
        {unit_pattern}\s*
        {currency_pattern}
        "
    ))
    .unwrap();
    for caps in amount_first.captures_iter(text) {
        let currency = normalize_currency(caps.name("currency")?.as_str());
        let amount = parse_amount_with_unit(
            caps.name("amount")?.as_str(),
            &currency,
            caps.name("unit").map(|item| item.as_str()),
        )?;
        if amount > 0.0 {
            return Some((amount, currency));
        }
    }

    let idr_unit_only = Regex::new(&format!(
        r"(?ix)
        (?P<amount>{amount_pattern})\s*
        (?P<unit>rb|ribu|jt|juta)
        "
    ))
    .unwrap();
    for caps in idr_unit_only.captures_iter(text) {
        let amount = parse_amount_with_unit(
            caps.name("amount")?.as_str(),
            "IDR",
            caps.name("unit").map(|item| item.as_str()),
        )?;
        if amount > 0.0 {
            return Some((amount, "IDR".to_string()));
        }
    }
    None
}

fn parse_amount_with_unit(value: &str, currency: &str, unit: Option<&str>) -> Option<f64> {
    let unit = unit.unwrap_or("");
    let amount = if unit.is_empty() {
        parse_amount(value, currency)?
    } else {
        parse_unit_amount(value)?
    };
    let multiplier = match unit.to_lowercase().as_str() {
        "rb" | "ribu" | "thousand" | "k" => 1_000.0,
        "jt" | "juta" | "million" | "m" => 1_000_000.0,
        _ => 1.0,
    };
    Some(amount * multiplier)
}

fn parse_unit_amount(value: &str) -> Option<f64> {
    let mut normalized = Regex::new(r"\s+")
        .unwrap()
        .replace_all(value.trim(), "")
        .to_string();
    if normalized.contains('.') && normalized.contains(',') {
        let last_dot = normalized.rfind('.').unwrap_or(0);
        let last_comma = normalized.rfind(',').unwrap_or(0);
        if last_dot > last_comma {
            normalized = normalized.replace(',', "");
        } else {
            normalized = normalized.replace('.', "").replace(',', ".");
        }
    } else if Regex::new(r"^\d{1,3}([.,]\d{3})+$")
        .unwrap()
        .is_match(&normalized)
    {
        normalized = normalized.replace(['.', ','], "");
    } else if normalized.matches(',').count() == 1 && normalized.matches('.').count() == 0 {
        normalized = normalized.replace(',', ".");
    }
    normalized.parse::<f64>().ok()
}

fn parse_amount(value: &str, currency: &str) -> Option<f64> {
    let zero_decimal = matches!(currency, "JPY" | "KRW" | "IDR" | "VND");
    let mut normalized = Regex::new(r"\s+")
        .unwrap()
        .replace_all(value.trim(), "")
        .to_string();
    if zero_decimal {
        if normalized.contains('.') && normalized.contains(',') {
            let last_dot = normalized.rfind('.').unwrap_or(0);
            let last_comma = normalized.rfind(',').unwrap_or(0);
            if last_dot > last_comma {
                normalized = normalized.replace(',', "");
                normalized = normalized.split('.').next().unwrap_or("").to_string();
            } else {
                normalized = normalized.replace('.', "");
                normalized = normalized.split(',').next().unwrap_or("").to_string();
            }
        } else if Regex::new(r"^\d{1,3}(\.\d{3})+$")
            .unwrap()
            .is_match(&normalized)
        {
            normalized = normalized.replace('.', "");
        } else if Regex::new(r"^\d{1,3}(,\d{3})+$")
            .unwrap()
            .is_match(&normalized)
        {
            normalized = normalized.replace(',', "");
        } else if normalized.contains('.') {
            normalized = normalized.split('.').next().unwrap_or("").to_string();
        } else if let Some((left, right)) = normalized.split_once(',') {
            if right.len() == 3 {
                normalized = format!("{left}{right}");
            } else {
                normalized = format!("{left}{}", &right[..right.len().min(3)]);
            }
        } else {
            normalized = normalized.replace(['.', ','], "");
        }
    } else if normalized.contains('.') && normalized.contains(',') {
        let last_dot = normalized.rfind('.').unwrap_or(0);
        let last_comma = normalized.rfind(',').unwrap_or(0);
        if last_dot > last_comma {
            normalized = normalized.replace(',', "");
        } else {
            normalized = normalized.replace('.', "").replace(',', ".");
        }
    } else if Regex::new(r"^\d{1,3}(,\d{3})+$")
        .unwrap()
        .is_match(&normalized)
    {
        normalized = normalized.replace(',', "");
    } else if normalized.matches(',').count() == 1 && normalized.matches('.').count() == 0 {
        normalized = normalized.replace(',', ".");
    } else {
        normalized = normalized.replace(',', "");
    }
    normalized.parse::<f64>().ok()
}

pub(crate) fn infer_currency(text: &str, from: &str) -> Option<String> {
    if let Some((_, currency)) = extract_amount(text) {
        return Some(currency);
    }
    let combined = format!("{text} {from}").to_lowercase();
    if Regex::new(r"(?i)\bidr\b|\brp\b|rp\.|rupiah|klikbca|bni\.co|bnicreditcard|kartukreditbca|layanankartukredit|mandiri|bri\.co|bca|cimb|danamon|permata|ovo|gopay|shopeepay|dana|linkaja")
        .unwrap()
        .is_match(&combined)
    {
        return Some("IDR".to_string());
    }
    if Regex::new(r"(?i)\bjpy\b|¥|円|yen|yucho|rakuten|smbc|mufg|mizuho|paypay")
        .unwrap()
        .is_match(&combined)
    {
        return Some("JPY".to_string());
    }
    if Regex::new(r"(?i)\busd\b|us\$|\$|dollars?|chase|bank of america|bofa|wells fargo|capital one|citi|amex|american express|venmo|cash app|paypal")
        .unwrap()
        .is_match(&combined)
    {
        return Some("USD".to_string());
    }
    None
}

fn normalize_currency(value: &str) -> String {
    match value.trim().to_uppercase().as_str() {
        "¥" | "円" | "YEN" => "JPY".to_string(),
        "$" | "US$" => "USD".to_string(),
        "DOLLAR" | "DOLLARS" => "USD".to_string(),
        "RP" | "RP." | "RUPIAH" => "IDR".to_string(),
        other => other.to_string(),
    }
}

fn extract_date(text: &str) -> Option<String> {
    let iso = Regex::new(r"\b(20\d{2})[-/](\d{1,2})[-/](\d{1,2})\b").unwrap();
    if let Some(caps) = iso.captures(text) {
        return format_date_parts(&caps[1], &caps[2], &caps[3]);
    }
    let dmy = Regex::new(r"\b(\d{1,2})[-/](\d{1,2})[-/](20\d{2})\b").unwrap();
    if let Some(caps) = dmy.captures(text) {
        return format_date_parts(&caps[3], &caps[2], &caps[1]);
    }
    None
}

fn format_date_parts(year: &str, month: &str, day: &str) -> Option<String> {
    let date = chrono::NaiveDate::from_ymd_opt(
        year.parse().ok()?,
        month.parse().ok()?,
        day.parse().ok()?,
    )?;
    Some(date.to_string())
}

fn non_finance_reason(message: &EmailMessage) -> Option<String> {
    let subject = message.subject.to_lowercase();
    let from = message.from.to_lowercase();
    let text = format!("{} {} {}", subject, message.snippet, message.body).to_lowercase();

    if Regex::new(r"(?i)jobstreet|e\.jobstreet|lowongan|peringatan pekerjaan|job alert|kandidat kuat|rekomendasi lina|careers?@|hiring@|noreply@e\.jobstreet")
        .unwrap()
        .is_match(&format!("{subject} {from} {text}"))
    {
        return Some("job alert or recruitment email".to_string());
    }

    if Regex::new(
        r"(?i)tripadvisor|trips to book|travel inspiration|inspiration@mp\d*\.tripadvisor",
    )
    .unwrap()
    .is_match(&format!("{subject} {from} {text}"))
    {
        return Some("travel marketing or inspiration".to_string());
    }

    if Regex::new(r"(?i)indodax|crypto trader|trader kripto|news update\]")
        .unwrap()
        .is_match(&format!("{subject} {from} {text}"))
        && Regex::new(r"(?i)gratis|promo|kesempatan|hack|berhasil dapat|mulai dari nol|giveaway|daftar sekarang")
            .unwrap()
            .is_match(&text)
    {
        return Some("crypto promo or news".to_string());
    }

    if Regex::new(r"(?i)trial ends|premium trial|add payment details before|update (your )?payment method before")
        .unwrap()
        .is_match(&text)
    {
        return Some("subscription or trial reminder".to_string());
    }

    if Regex::new(r"(?i)informasi layanan|cuti bersama|libur nasional|holiday hours|service (?:notice|information)")
        .unwrap()
        .is_match(&text)
    {
        return Some("service notice".to_string());
    }

    if Regex::new(
        r"(?i)kenaikan\s+limit|peningkatan\s+limit|limit\s+sementara|permohonan\s+kenaikan|penawaran\s+limit|pengajuan\s+limit|credit\s+limit\s+(?:increase|offer)|temporary\s+(?:credit\s+)?limit|informasi\s+permohonan\s+kenaikan",
    )
    .unwrap()
    .is_match(&format!("{subject} {text}"))
    {
        return Some("credit card limit or service notice".to_string());
    }

    if Regex::new(
        r"(?i)cicilan\s+(?:bca\s+)?0\s*%|0\s*%\s+(?:cicilan|di\s+mybca)|transaksi\s+jadi\s+ringan|penawaran\s+cicilan|program\s+cicilan|aktifkan\s+cicilan|konversi\s+(?:ke\s+)?cicilan|mybca.*ubah\s+sekarang|cicilan.*ubah\s+sekarang",
    )
    .unwrap()
    .is_match(&format!("{subject} {text}"))
    {
        return Some("credit card installment promotion".to_string());
    }

    if Regex::new(r"(?i)e-billing|billing statement|tanggal cetak|statement (?:date|period)|ringkasan tagihan|minimum payment due")
        .unwrap()
        .is_match(&text)
        && !has_transaction_signal(&text)
    {
        return Some("billing statement notice".to_string());
    }

    if is_marketing(&text) && !has_transaction_signal(&text) {
        return Some("marketing or promotion".to_string());
    }

    None
}

fn has_transaction_signal(text: &str) -> bool {
    if has_strong_transaction_signal(text) {
        return true;
    }
    Regex::new(
        r"(?i)notifikasi\s+transaksi|transaksi\s+kartu|di\s+merchant|merchant\s+[a-z0-9][a-z0-9.-]*\.[a-z]{2,}|(?:mastercard|visa)[xX\d]{4,}|kartu\s+(?:mastercard|visa)|bnicreditcard@",
    )
    .unwrap()
    .is_match(text)
}

fn has_strong_transaction_signal(text: &str) -> bool {
    Regex::new(
        r"(?i)(?:has been|were|was)\s+(?:debited|credited|charged|paid)|successful(?:ly)?\s+(?:transferred|sent|received|processed)|transaction\s+(?:alert|notification|successful|complete)|transfer\s+(?:successful|completed)|debit\s+alert|credit\s+alert|pembayaran\s+berhasil|transaksi\s+berhasil|payment\s+(?:received|successful|completed)|you\s+(?:paid|sent|received|transferred)|paid\s+to\s+|received\s+from\s+|card\s+ending\s+\d{4}.*(?:charged|debited)",
    )
    .unwrap()
    .is_match(text)
}

fn merchant_from_subject(subject: &str) -> Option<String> {
    Regex::new(r"(?i)di\s+merchant\s+([^\s,]+)")
        .unwrap()
        .captures(subject)
        .and_then(|caps| caps.get(1).map(|item| item.as_str().trim().to_string()))
        .filter(|value| !value.is_empty())
}

fn is_promotional_amount_context(text: &str) -> bool {
    Regex::new(
        r"(?i)gratis|giveaway|kesempatan\s+dapat|dapat\s+rp|win\s+rp|hack\s+sistem|news\s+update|lowongan|kandidat\s+kuat|trips\s+to\s+book|trial\s+ends|cashback\s+offer|diskon|promo(?:tion)?|referral|mulai\s+dari\s+nol|software\s+engineer\s+\[|cicilan\s+(?:bca\s+)?0\s*%|transaksi\s+jadi\s+ringan|ubah\s+sekarang",
    )
    .unwrap()
    .is_match(text)
}

fn is_marketing(text: &str) -> bool {
    Regex::new(
        r"(?i)earn|join now|cashback offer|newsletter|special offer|limited time|diskon|referral|friends join|unsubscribe|view in browser|lowongan baru|peringatan pekerjaan|job alert|kandidat kuat|trips to book|inspiration@|noreply@e\.jobstreet|gratis\s*\+|kesempatan\s+dapat",
    )
    .unwrap()
    .is_match(text)
}

fn merchant_from_sender(from: &str, subject: &str) -> String {
    let display = Regex::new(r#"^"?([^"<]+)"?\s*<"#)
        .unwrap()
        .captures(from)
        .and_then(|caps| caps.get(1).map(|item| item.as_str().trim().to_string()));
    display.filter(|item| !item.is_empty()).unwrap_or_else(|| {
        extract_email_address(from)
            .split('@')
            .nth(1)
            .and_then(|domain| domain.split('.').next())
            .unwrap_or_else(|| subject.split_whitespace().next().unwrap_or("Unknown"))
            .to_string()
    })
}

fn account_from_text(text: &str, from: &str) -> String {
    if let Some(caps) = Regex::new(r"(?i)(?:kartu\s+)?((?:mastercard|visa)[xX\d]{4,})")
        .unwrap()
        .captures(text)
    {
        return caps[1].to_string();
    }
    if let Some(caps) = Regex::new(r"(?i)(?:source of fund|source account|nomor kartu|account|rekening|card)\s*[:\-]?\s*([A-Z0-9xX*]{4,})")
        .unwrap()
        .captures(text)
    {
        return caps[1].to_string();
    }
    merchant_from_sender(from, "")
}

fn is_retryable_ai_error(value: &str) -> bool {
    Regex::new(r"(?i)quota|rate|limit|429|503|temporar|timeout|unavailable|too many|fetch|request too large|tokens per minute|not set")
        .unwrap()
        .is_match(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_indonesian_rupiah_formats() {
        assert_eq!(
            extract_amount("Total Rp12.500"),
            Some((12_500.0, "IDR".to_string()))
        );
        assert_eq!(
            extract_amount("Jumlah 12.500 rupiah"),
            Some((12_500.0, "IDR".to_string()))
        );
        assert_eq!(
            extract_amount("Tagihan 1,5 juta"),
            Some((1_500_000.0, "IDR".to_string()))
        );
    }

    #[test]
    fn extracts_japanese_yen_formats() {
        assert_eq!(
            extract_amount("Paid ¥1,000"),
            Some((1_000.0, "JPY".to_string()))
        );
        assert_eq!(
            extract_amount("合計 1,000円"),
            Some((1_000.0, "JPY".to_string()))
        );
        assert_eq!(
            extract_amount("Total 1000 yen"),
            Some((1_000.0, "JPY".to_string()))
        );
    }

    #[test]
    fn extracts_american_dollar_formats() {
        assert_eq!(
            extract_amount("Charged $12.34"),
            Some((12.34, "USD".to_string()))
        );
        assert_eq!(
            extract_amount("Total US$ 1,234.56"),
            Some((1_234.56, "USD".to_string()))
        );
        assert_eq!(
            extract_amount("Paid 12.34 dollars"),
            Some((12.34, "USD".to_string()))
        );
    }
}
