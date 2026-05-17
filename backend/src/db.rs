use std::env;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::{PgPool, Row};

use crate::finance;
use crate::models::{
    DashboardResponse, EmailAnalysis, EmailMessage, SpendingTrendPoint, SpendingTrendResponse,
    TransactionRow,
};
use crate::utils::{
    extract_email_address, month_range_bounds, parse_date, postgres_safe_opt, postgres_safe_text,
};

pub(crate) async fn connect() -> Result<PgPool> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://finance:finance@127.0.0.1:5432/finance_radar".to_string());
    PgPool::connect(&database_url)
        .await
        .context("connect database")
}

pub(crate) async fn save_results(
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
                    .or_else(|| finance::infer_currency(&message_text, &message.from))
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

pub(crate) async fn load_dashboard(
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

pub(crate) async fn load_spending_trend(
    db: &PgPool,
    from: &str,
    to: &str,
) -> Result<SpendingTrendResponse> {
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

fn row_f64(row: &sqlx::postgres::PgRow, column: &str) -> Option<f64> {
    row.try_get::<f64, _>(column).ok()
}
