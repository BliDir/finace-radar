use std::{env, net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod ai;
mod db;
mod finance;
mod gmail;
mod models;
mod utils;

use ai::AiConfig;
use models::{DashboardResponse, SpendingTrendResponse};

#[derive(Clone)]
struct AppState {
    db: PgPool,
    http: Client,
    ai: AiConfig,
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

    let state = Arc::new(AppState {
        db: db::connect().await?,
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
    let month = query.month.unwrap_or_else(utils::current_month);
    Ok(Json(db::load_dashboard(&state.db, &month, None).await?))
}

async fn spending_trend(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendQuery>,
) -> Result<Json<SpendingTrendResponse>, AppError> {
    Ok(Json(
        db::load_spending_trend(&state.db, &query.from, &query.to).await?,
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
        let messages = gmail::fetch_messages(&state.http, token, &request.month).await?;
        let analyses = finance::analyze_messages(&state.http, &state.ai, &messages).await?;
        db::save_results(&state.db, &request.month, &messages, &analyses).await?;
    }

    Ok(Json(
        db::load_dashboard(&state.db, &request.month, request.email).await?,
    ))
}
