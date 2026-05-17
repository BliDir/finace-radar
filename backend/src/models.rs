use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EmailMessage {
    pub(crate) id: String,
    pub(crate) from: String,
    pub(crate) subject: String,
    pub(crate) date: String,
    pub(crate) snippet: String,
    pub(crate) body: String,
    pub(crate) timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EmailAnalysis {
    pub(crate) id: String,
    #[serde(rename = "isFinance")]
    pub(crate) is_finance: bool,
    pub(crate) direction: String,
    pub(crate) amount: Option<f64>,
    pub(crate) currency: Option<String>,
    pub(crate) date: Option<String>,
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) account: Option<String>,
    #[serde(rename = "accountType")]
    pub(crate) account_type: Option<String>,
    pub(crate) merchant: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) confidence: String,
}

#[derive(Serialize)]
pub(crate) struct DashboardResponse {
    pub(crate) month: String,
    pub(crate) email: Option<String>,
    pub(crate) emails: Vec<EmailMessage>,
    pub(crate) analyses: Vec<EmailAnalysis>,
    pub(crate) transactions: Vec<TransactionRow>,
}

#[derive(Serialize)]
pub(crate) struct TransactionRow {
    pub(crate) id: String,
    pub(crate) merchant: String,
    pub(crate) date: String,
    pub(crate) amount: f64,
    pub(crate) currency: String,
    #[serde(rename = "isFinance")]
    pub(crate) is_finance: bool,
    pub(crate) direction: String,
    #[serde(rename = "fromParty")]
    pub(crate) from_party: String,
    #[serde(rename = "toParty")]
    pub(crate) to_party: String,
    pub(crate) account: String,
    #[serde(rename = "accountConfidence")]
    pub(crate) account_confidence: String,
    pub(crate) category: String,
    pub(crate) source: String,
    #[serde(rename = "nextRenewal")]
    pub(crate) next_renewal: Option<String>,
    pub(crate) recurring: bool,
}

#[derive(Serialize)]
pub(crate) struct SpendingTrendPoint {
    pub(crate) month: String,
    pub(crate) spending: f64,
}

#[derive(Serialize)]
pub(crate) struct SpendingTrendResponse {
    pub(crate) points: Vec<SpendingTrendPoint>,
}
