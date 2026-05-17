use std::{env, net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    http: Client,
    ai: AiConfig,
}

#[derive(Clone)]
struct AiConfig {
    providers: Vec<String>,
    gemini_key: String,
    gemini_model: String,
    groq_key: String,
    groq_model: String,
    ollama_base_url: String,
    ollama_model: String,
}

#[derive(Deserialize)]
struct MonthQuery {
    month: Option<String>,
}

#[derive(Deserialize)]
struct TrendQuery {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct SpendingTrendPoint {
    month: String,
    spending: f64,
}

#[derive(Serialize)]
struct SpendingTrendResponse {
    points: Vec<SpendingTrendPoint>,
}

#[derive(Deserialize)]
struct ReadInboxRequest {
    month: String,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    email: Option<String>,
}

#[derive(Serialize)]
struct ConfigResponse {
    #[serde(rename = "aiProvider")]
    ai_provider: String,
    #[serde(rename = "aiModel")]
    ai_model: String,
    #[serde(rename = "aiProviders")]
    ai_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailMessage {
    id: String,
    from: String,
    subject: String,
    date: String,
    snippet: String,
    body: String,
    timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailAnalysis {
    id: String,
    #[serde(rename = "isFinance")]
    is_finance: bool,
    direction: String,
    amount: Option<f64>,
    currency: Option<String>,
    date: Option<String>,
    from: Option<String>,
    to: Option<String>,
    account: Option<String>,
    #[serde(rename = "accountType")]
    account_type: Option<String>,
    merchant: Option<String>,
    category: Option<String>,
    confidence: String,
}

#[derive(Serialize)]
struct DashboardResponse {
    month: String,
    email: Option<String>,
    emails: Vec<EmailMessage>,
    analyses: Vec<EmailAnalysis>,
    transactions: Vec<TransactionRow>,
}

#[derive(Serialize)]
struct TransactionRow {
    id: String,
    merchant: String,
    date: String,
    amount: f64,
    currency: String,
    #[serde(rename = "isFinance")]
    is_finance: bool,
    direction: String,
    #[serde(rename = "fromParty")]
    from_party: String,
    #[serde(rename = "toParty")]
    to_party: String,
    account: String,
    #[serde(rename = "accountConfidence")]
    account_confidence: String,
    category: String,
    source: String,
    #[serde(rename = "nextRenewal")]
    next_renewal: Option<String>,
    recurring: bool,
}

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

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": message })),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://finance:finance@127.0.0.1:5432/finance_radar".to_string());
    let db = PgPool::connect(&database_url)
        .await
        .context("connect database")?;
    let state = Arc::new(AppState {
        db,
        http: Client::builder().build()?,
        ai: AiConfig::from_env(),
    });

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/config", get(config))
        .route("/api/dashboard", get(dashboard))
        .route("/api/spending-trend", get(spending_trend))
        .route("/api/read-inbox", post(read_inbox))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(4000);
    let address: SocketAddr = format!("{host}:{port}").parse()?;
    info!("backend listening on {address}");
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

impl AiConfig {
    fn from_env() -> Self {
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

    fn primary(&self) -> (&str, &str) {
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
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let (provider, model) = state.ai.primary();
    Json(ConfigResponse {
        ai_provider: provider.to_string(),
        ai_model: model.to_string(),
        ai_providers: state.ai.providers.clone(),
    })
}

async fn dashboard(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MonthQuery>,
) -> Result<Json<DashboardResponse>, AppError> {
    let month = query.month.unwrap_or_else(current_month);
    Ok(Json(load_dashboard(&state.db, &month, None).await?))
}

async fn spending_trend(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendQuery>,
) -> Result<Json<SpendingTrendResponse>, AppError> {
    Ok(Json(
        load_spending_trend(&state.db, &query.from, &query.to).await?,
    ))
}

async fn read_inbox(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ReadInboxRequest>,
) -> Result<Json<DashboardResponse>, AppError> {
    if let Some(token) = request
        .access_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        let messages = fetch_gmail_messages(&state.http, token, &request.month).await?;
        let analyses = analyze_with_fallback(&state, &messages).await?;
        save_results(&state.db, &request.month, &messages, &analyses).await?;
    }

    Ok(Json(
        load_dashboard(&state.db, &request.month, request.email).await?,
    ))
}

async fn fetch_gmail_messages(
    http: &Client,
    token: &str,
    month: &str,
) -> Result<Vec<EmailMessage>> {
    let query = gmail_query_for_month(month)?;
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
        messages.push(normalize_gmail_message(
            response.json::<GmailMessageResponse>().await?,
        )?);
    }
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(messages)
}

fn gmail_query_for_month(month: &str) -> Result<String> {
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

fn normalize_gmail_message(message: GmailMessageResponse) -> Result<EmailMessage> {
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

async fn analyze_with_fallback(
    state: &AppState,
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
        for provider in &state.ai.providers {
            let attempt = match provider.as_str() {
                "gemini" => call_gemini(state, message).await,
                "groq" => call_groq(state, message).await,
                "ollama" => call_ollama(state, message).await,
                other => Err(anyhow!("unknown AI provider {other}")),
            };

            match attempt {
                Ok(mut item) => {
                    provider_used = provider.clone();
                    model_used = model_for_provider(&state.ai, provider).to_string();
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

async fn call_gemini(state: &AppState, message: &EmailMessage) -> Result<EmailAnalysis> {
    if state.ai.gemini_key.is_empty() {
        return Err(anyhow!("GEMINI_API_KEY is not set"));
    }
    let prompt = prompt_for_email(message);
    let response = state
        .http
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            state.ai.gemini_model
        ))
        .header("x-goog-api-key", &state.ai.gemini_key)
        .json(&json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
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
    parse_analysis_json(text, &message.id)
}

async fn call_groq(state: &AppState, message: &EmailMessage) -> Result<EmailAnalysis> {
    if state.ai.groq_key.is_empty() {
        return Err(anyhow!("GROQ_API_KEY is not set"));
    }
    let response = state
        .http
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(&state.ai.groq_key)
        .json(&json!({
            "model": state.ai.groq_model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "Return strict JSON only." },
                { "role": "user", "content": prompt_for_email(message) }
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
    parse_analysis_json(text, &message.id)
}

async fn call_ollama(state: &AppState, message: &EmailMessage) -> Result<EmailAnalysis> {
    let response = state
        .http
        .post(format!(
            "{}/api/chat",
            state.ai.ollama_base_url.trim_end_matches('/')
        ))
        .json(&json!({
            "model": state.ai.ollama_model,
            "stream": false,
            "format": "json",
            "messages": [
                { "role": "system", "content": "Return strict JSON only." },
                { "role": "user", "content": prompt_for_email(message) }
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
    parse_analysis_json(text, &message.id)
}

fn prompt_for_email(message: &EmailMessage) -> String {
    format!(
        r#"Analyze this single email for personal finance transaction data.

Return JSON only with this shape:
{{"analyses":[{{"id":"{id}","isFinance":true|false,"direction":"spending|income|transfer|refund|fee|non_finance","amount":number|null,"currency":"IDR|JPY|USD|...|null","date":"YYYY-MM-DD|null","from":"payer/source|null","to":"payee/recipient|null","account":"bank/card/wallet/account identifier|null","accountType":"bank_account|credit_card|debit_card|wallet|unknown|null","merchant":"merchant or institution|null","category":"category|null","confidence":"high|medium|low"}}]}}

Rules:
- Return exactly one analysis for the input id. Do not use facts from other emails.
- A completed money movement is finance even if it is from a no-reply address, has emoji, or is formatted like a notification.
- Treat successful transfers, card charges, debit/credit alerts, receipts, invoices, paid bills, refunds, and bank journals as finance.
- Indonesian card alerts such as "Notifikasi Transaksi Kartu MASTERCARD... di merchant Netflix.com" from BNI or other banks are completed spending.
- Extract parties from labels like source of fund, source account, beneficiary, recipient, merchant, payee, payer, from, to, account, card, rekening, kartu, penerima, merchant/ATM.
- Extract amount and currency from labels like amount, total, charged, paid, debit, credit, transfer amount, transaction amount, sejumlah, nilai, nominal, jumlah, tagihan.
- Recognize Indonesian rupiah as IDR from IDR, Rp, Rp., rupiah, rb/ribu, jt/juta, and formats such as Rp12.500, IDR 12,500, 12.500 rupiah, 600 ribu, 1,5 juta.
- Recognize Japanese yen as JPY from JPY, ¥, 円, yen, and formats such as ¥1,000, JPY 1000, 1,000円, 1000 yen.
- Recognize American dollars as USD from USD, US$, $, dollar, dollars, and formats such as $12.34, US$ 12.34, USD 12.34, 12.34 dollars.
- For JPY, KRW, IDR, and VND, separators are usually thousands separators and the currency has no cents. USD commonly has cents.
- Exclude marketing, referral rewards, cashback offers, newsletters, promotions, ads, job alerts, travel inspiration, crypto promos, news articles, trial/payment-method reminders, and generic service notices when there is no completed transaction amount.
- Job alerts (Jobstreet, LinkedIn jobs, "lowongan", "kandidat kuat"), travel marketing (Tripadvisor inspiration), crypto promos (Indodax giveaways), and "add payment details before trial ends" are never finance.
- Credit card e-billing statements that only show a statement balance or due date without a new charge event are not finance.
- Credit card service notices such as temporary limit increases ("Kenaikan Limit Sementara", "Informasi Permohonan") are not finance.
- Credit card installment promos such as "Cicilan BCA 0%" or "Transaksi Jadi Ringan" are marketing, not completed spending.
- Promo amounts like "win Rp600 ribu" or headline figures in news are not transaction amounts.
- If no actual transaction exists, set isFinance false, direction non_finance, amount null.

Email:
id: {id}
from: {from}
subject: {subject}
date: {date}
snippet: {snippet}
body:
{body}
"#,
        id = message.id,
        from = message.from,
        subject = message.subject,
        date = message.date,
        snippet = message.snippet,
        body = compact_email_for_analysis(&message.body)
    )
}

fn parse_analysis_json(text: &str, expected_id: &str) -> Result<EmailAnalysis> {
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

async fn save_results(
    db: &PgPool,
    month: &str,
    messages: &[EmailMessage],
    analyses: &[EmailAnalysis],
) -> Result<()> {
    for message in messages {
        let email_id: i64 = sqlx::query(
            r#"
            INSERT INTO emails (gmail_message_id, month, sender, subject, received_date, snippet, body, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, now())
            ON CONFLICT (gmail_message_id) DO UPDATE SET
                month = EXCLUDED.month,
                sender = EXCLUDED.sender,
                subject = EXCLUDED.subject,
                received_date = EXCLUDED.received_date,
                snippet = EXCLUDED.snippet,
                body = EXCLUDED.body,
                updated_at = now()
            RETURNING id
            "#,
        )
        .bind(postgres_safe_text(&message.id))
        .bind(month)
        .bind(postgres_safe_text(&message.from))
        .bind(postgres_safe_text(&message.subject))
        .bind(parse_date(&message.date))
        .bind(postgres_safe_text(&message.snippet))
        .bind(postgres_safe_text(&message.body))
        .fetch_one(db)
        .await?
        .get("id");

        sqlx::query("DELETE FROM email_analyses WHERE email_id = $1")
            .bind(email_id)
            .execute(db)
            .await?;

        if let Some(analysis) = analyses.iter().find(|item| item.id == message.id) {
            let message_text = format!("{} {} {}", message.subject, message.snippet, message.body);
            let provider = "ai";
            let model = "fallback";
            let analysis_id: i64 = sqlx::query(
                r#"
                INSERT INTO email_analyses
                    (email_id, provider, model, is_finance, direction, amount, currency, from_party,
                     to_party, account, account_type, merchant, category, confidence, raw_result)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
                RETURNING id
                "#,
            )
            .bind(email_id)
            .bind(provider)
            .bind(model)
            .bind(analysis.is_finance)
            .bind(postgres_safe_text(&analysis.direction))
            .bind(analysis.amount)
            .bind(postgres_safe_opt(&analysis.currency))
            .bind(postgres_safe_opt(&analysis.from))
            .bind(postgres_safe_opt(&analysis.to))
            .bind(postgres_safe_opt(&analysis.account))
            .bind(postgres_safe_opt(&analysis.account_type))
            .bind(postgres_safe_opt(&analysis.merchant))
            .bind(postgres_safe_opt(&analysis.category))
            .bind(postgres_safe_text(&analysis.confidence))
            .bind(serde_json::to_value(analysis)?)
            .fetch_one(db)
            .await?
            .get("id");

            if analysis.is_finance && analysis.amount.unwrap_or(0.0) > 0.0 {
                let currency = analysis
                    .currency
                    .clone()
                    .or_else(|| infer_currency(&message_text, &message.from))
                    .unwrap_or_else(|| "IDR".to_string());
                sqlx::query(
                    r#"
                    INSERT INTO transactions
                        (email_analysis_id, transaction_date, direction, amount, currency, from_party,
                         to_party, account, merchant, category)
                    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                    "#,
                )
                .bind(analysis_id)
                .bind(parse_date(analysis.date.as_deref().unwrap_or(&message.date)))
                .bind(postgres_safe_text(&analysis.direction))
                .bind(analysis.amount.unwrap_or(0.0))
                .bind(postgres_safe_text(&currency))
                .bind(postgres_safe_opt(&analysis.from))
                .bind(postgres_safe_opt(&analysis.to))
                .bind(postgres_safe_opt(&analysis.account))
                .bind(postgres_safe_opt(&analysis.merchant))
                .bind(postgres_safe_opt(&analysis.category))
                .execute(db)
                .await?;
            }
        }
    }
    Ok(())
}

async fn load_dashboard(
    db: &PgPool,
    month: &str,
    email: Option<String>,
) -> Result<DashboardResponse> {
    let email_rows = sqlx::query(
        r#"
        SELECT gmail_message_id, sender, subject, received_date, snippet, body,
               extract(epoch from updated_at) * 1000 as timestamp
        FROM emails
        WHERE month = $1
        ORDER BY received_date DESC NULLS LAST, id DESC
        "#,
    )
    .bind(month)
    .fetch_all(db)
    .await?;

    let emails = email_rows
        .iter()
        .map(|row| EmailMessage {
            id: row.get("gmail_message_id"),
            from: row.get("sender"),
            subject: row.get("subject"),
            date: row
                .try_get::<Option<NaiveDate>, _>("received_date")
                .ok()
                .flatten()
                .map(|date| date.to_string())
                .unwrap_or_default(),
            snippet: row.get("snippet"),
            body: row.get("body"),
            timestamp: row
                .try_get::<Option<f64>, _>("timestamp")
                .ok()
                .flatten()
                .unwrap_or(0.0) as i64,
        })
        .collect::<Vec<_>>();

    let analysis_rows = sqlx::query(
        r#"
        SELECT e.gmail_message_id, a.is_finance, a.direction,
               a.amount::double precision AS amount, a.currency,
               a.from_party, a.to_party, a.account, a.account_type, a.merchant,
               a.category, a.confidence, e.received_date
        FROM email_analyses a
        JOIN emails e ON e.id = a.email_id
        WHERE e.month = $1
        ORDER BY e.received_date DESC NULLS LAST, a.id DESC
        "#,
    )
    .bind(month)
    .fetch_all(db)
    .await?;

    let analyses = analysis_rows
        .iter()
        .map(|row| EmailAnalysis {
            id: row.get("gmail_message_id"),
            is_finance: row.get("is_finance"),
            direction: row.get("direction"),
            amount: row_f64(row, "amount"),
            currency: row.try_get("currency").ok(),
            date: row
                .try_get::<Option<NaiveDate>, _>("received_date")
                .ok()
                .flatten()
                .map(|date| date.to_string()),
            from: row.try_get("from_party").ok(),
            to: row.try_get("to_party").ok(),
            account: row.try_get("account").ok(),
            account_type: row.try_get("account_type").ok(),
            merchant: row.try_get("merchant").ok(),
            category: row.try_get("category").ok(),
            confidence: row.get("confidence"),
        })
        .collect::<Vec<_>>();

    let transaction_rows = sqlx::query(
        r#"
        SELECT t.id, t.transaction_date, t.direction,
               t.amount::double precision AS amount, t.currency, t.from_party,
               t.to_party, t.account, t.merchant, t.category, e.sender
        FROM transactions t
        JOIN email_analyses a ON a.id = t.email_analysis_id
        JOIN emails e ON e.id = a.email_id
        WHERE to_char(t.transaction_date, 'YYYY-MM') = $1
           OR (t.transaction_date IS NULL AND e.month = $1)
        ORDER BY t.transaction_date DESC NULLS LAST, t.id DESC
        "#,
    )
    .bind(month)
    .fetch_all(db)
    .await?;

    let transactions = transaction_rows
        .iter()
        .map(|row| {
            let merchant = row
                .try_get::<Option<String>, _>("merchant")
                .ok()
                .flatten()
                .unwrap_or_else(|| "Unknown".to_string());
            TransactionRow {
                id: format!("txn-{}", row.get::<i64, _>("id")),
                merchant: merchant.clone(),
                date: row
                    .try_get::<Option<NaiveDate>, _>("transaction_date")
                    .ok()
                    .flatten()
                    .map(|date| date.to_string())
                    .unwrap_or_default(),
                amount: row_f64(row, "amount").unwrap_or(0.0),
                currency: row
                    .try_get::<Option<String>, _>("currency")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "IDR".to_string()),
                is_finance: true,
                direction: row.get("direction"),
                from_party: row
                    .try_get::<Option<String>, _>("from_party")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Unknown".to_string()),
                to_party: row
                    .try_get::<Option<String>, _>("to_party")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| merchant.clone()),
                account: row
                    .try_get::<Option<String>, _>("account")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Unspecified account".to_string()),
                account_confidence: "medium".to_string(),
                category: row
                    .try_get::<Option<String>, _>("category")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Uncategorized".to_string()),
                source: extract_email_address(&row.get::<String, _>("sender")),
                next_renewal: None,
                recurring: false,
            }
        })
        .collect::<Vec<_>>();

    Ok(DashboardResponse {
        month: month.to_string(),
        email,
        emails,
        analyses,
        transactions,
    })
}

async fn load_spending_trend(db: &PgPool, from: &str, to: &str) -> Result<SpendingTrendResponse> {
    let (start, end) = month_range_bounds(from, to)?;
    let rows = sqlx::query(
        r#"
        SELECT to_char(t.transaction_date, 'YYYY-MM') AS month,
               COALESCE(SUM(t.amount::double precision), 0) AS spending
        FROM transactions t
        WHERE t.transaction_date >= $1
          AND t.transaction_date < $2
          AND t.direction IN ('spending', 'fee')
        GROUP BY 1
        ORDER BY 1
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await?;

    let points = rows
        .iter()
        .map(|row| SpendingTrendPoint {
            month: row.get("month"),
            spending: row_f64(row, "spending").unwrap_or(0.0),
        })
        .collect();

    Ok(SpendingTrendResponse { points })
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

fn decode_base64_url(value: &str) -> Result<String> {
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| general_purpose::URL_SAFE.decode(value))
        .or_else(|_| general_purpose::STANDARD.decode(value))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn parse_raw_headers(raw: &str) -> std::collections::HashMap<String, String> {
    let header_text = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(h, _)| h)
        .unwrap_or("");
    let unfolded = Regex::new(r"\r?\n[ \t]+")
        .unwrap()
        .replace_all(header_text, " ");
    let mut headers = std::collections::HashMap::new();
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

fn row_f64(row: &sqlx::postgres::PgRow, column: &str) -> Option<f64> {
    row.try_get::<f64, _>(column).ok()
}

/// PostgreSQL rejects NUL bytes in UTF-8 text; email MIME decoding can produce them.
fn postgres_safe_text(value: &str) -> String {
    value.replace('\0', "")
}

fn postgres_safe_opt(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|item| postgres_safe_text(item))
}

fn clean_text(value: &str) -> String {
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

fn compact_email_for_analysis(body: &str) -> String {
    if body.len() <= 5000 {
        return body.to_string();
    }
    let pattern = Regex::new(
        r"(?i)amount|total|charged|paid|payment|debit|credit|transfer|transaction|status|successful|merchant|beneficiary|recipient|source|account|card|currency|date|reference|sejumlah|nominal|jumlah|nilai|tagihan|transaksi|pembayaran|rekening|kartu|penerima|tanggal",
    )
    .unwrap();
    let lines = body.lines().collect::<Vec<_>>();
    let mut selected = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if pattern.is_match(line) {
            if index > 0 {
                selected.push(lines[index - 1]);
            }
            selected.push(line);
            if index + 1 < lines.len() {
                selected.push(lines[index + 1]);
            }
        }
    }
    let compact = clean_text(&selected.join("\n"));
    if compact.len() > 500 {
        compact.chars().take(5000).collect()
    } else {
        body.chars().take(5000).collect()
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

fn infer_currency(text: &str, from: &str) -> Option<String> {
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
    let date = NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)?;
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

fn extract_email_address(value: &str) -> String {
    Regex::new(r"<([^>]+)>")
        .unwrap()
        .captures(value)
        .and_then(|caps| caps.get(1).map(|item| item.as_str().to_string()))
        .unwrap_or_else(|| value.trim().to_string())
}

fn is_retryable_ai_error(value: &str) -> bool {
    Regex::new(r"(?i)quota|rate|limit|429|503|temporar|timeout|unavailable|too many|fetch|request too large|tokens per minute|not set")
        .unwrap()
        .is_match(value)
}

fn model_for_provider<'a>(ai: &'a AiConfig, provider: &str) -> &'a str {
    match provider {
        "groq" => &ai.groq_model,
        "ollama" => &ai.ollama_model,
        _ => &ai.gemini_model,
    }
}

fn parse_month(month: &str) -> Result<(i32, u32)> {
    let (year, month) = month.split_once('-').context("month must be YYYY-MM")?;
    Ok((year.parse()?, month.parse()?))
}

fn month_range_bounds(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate)> {
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

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn current_month() -> String {
    let now = Utc::now().date_naive();
    format!("{:04}-{:02}", now.year(), now.month())
}

fn current_date() -> String {
    Utc::now().date_naive().to_string()
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
