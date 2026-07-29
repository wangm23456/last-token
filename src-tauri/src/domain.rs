use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Claude,
    Codex,
    Gemini,
    Copilot,
    Kimi,
    Zhipu,
    ZhipuTeam,
    Minimax,
    Zenmux,
    Volcengine,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Mirrors the snake_case serde repr used by storage & the frontend type.
        f.write_str(match self {
            ProviderKind::Claude => "claude",
            ProviderKind::Codex => "codex",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Copilot => "copilot",
            ProviderKind::Kimi => "kimi",
            ProviderKind::Zhipu => "zhipu",
            ProviderKind::ZhipuTeam => "zhipu_team",
            ProviderKind::Minimax => "minimax",
            ProviderKind::Zenmux => "zenmux",
            ProviderKind::Volcengine => "volcengine",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    CliAuto,
    Env,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Valid,
    Expired,
    NotFound,
    ParseError,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskState {
    Safe,
    AtRisk,
    Exhausted,
    Learning,
    UnknownReset,
    Error,
}

/// Explicit business severity. Declaration order of the enum is NOT
/// meaningful: exhausted > at_risk > unknown_reset > error > learning > safe.
pub fn risk_severity(state: RiskState) -> i32 {
    match state {
        RiskState::Exhausted => 5,
        RiskState::AtRisk => 4,
        RiskState::UnknownReset => 3,
        RiskState::Error => 2,
        RiskState::Learning => 1,
        RiskState::Safe => 0,
    }
}

/// Stable display ordering for tiers: short session windows first, then
/// weekly/model-scoped windows, then monthly, then anything unknown by id.
pub fn tier_sort_key(tier_id: &str) -> (u8, String) {
    let rank = match tier_id {
        "five_hour" => 0,
        "seven_day" | "weekly_limit" => 1,
        id if id.starts_with("gemini_") => 1,
        "monthly" | "30_day" => 2,
        _ => 3,
    };
    (rank, tier_id.to_string())
}

/// Pick the leading (worst) tier: highest severity, tie broken by higher
/// utilization, then by the earlier exhaustion/reset time.
pub fn leading_tier<'a>(tiers: &'a [TierDashboard]) -> Option<&'a TierDashboard> {
    tiers.iter().max_by(|a, b| {
        let sa = risk_severity(a.forecast.state);
        let sb = risk_severity(b.forecast.state);
        sa.cmp(&sb)
            .then_with(|| {
                a.quota
                    .utilization
                    .partial_cmp(&b.quota.utilization)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let ta = a.forecast.exhaustion_at.or(a.quota.resets_at).unwrap_or(i64::MAX);
                let tb = b.forecast.exhaustion_at.or(b.quota.resets_at).unwrap_or(i64::MAX);
                // earlier time wins -> reverse
                tb.cmp(&ta)
            })
    })
}

fn account_risk_severity(acc: &AccountDashboard) -> f64 {
    if acc.credential_status != CredentialStatus::Valid || acc.error.is_some() {
        // Account-level error: between unknown_reset and at_risk.
        return f64::from(risk_severity(RiskState::UnknownReset)) + 0.5;
    }
    match leading_tier(&acc.tiers) {
        Some(tier) => f64::from(risk_severity(tier.forecast.state)),
        None => f64::from(risk_severity(RiskState::Safe)),
    }
}

fn account_earliest_time(acc: &AccountDashboard) -> i64 {
    let mut min = i64::MAX;
    for tier in &acc.tiers {
        if let Some(t) = tier.forecast.exhaustion_at.or(tier.quota.resets_at) {
            if t < min {
                min = t;
            }
        }
    }
    min
}

/// Higher severity first; same severity uses earliest exhaustion/reset.
pub fn sort_accounts_by_risk(accounts: &[AccountDashboard]) -> Vec<AccountDashboard> {
    let mut ordered = accounts.to_vec();
    ordered.sort_by(|a, b| {
        let sev = account_risk_severity(b)
            .partial_cmp(&account_risk_severity(a))
            .unwrap_or(std::cmp::Ordering::Equal);
        sev.then_with(|| account_earliest_time(a).cmp(&account_earliest_time(b)))
    });
    ordered
}

/// Manual order wins when present; otherwise sort by risk severity.
/// Unknown/new ids not in `order` are appended in risk order.
pub fn apply_account_order(accounts: &[AccountDashboard], order: &[String]) -> Vec<AccountDashboard> {
    let by_risk = sort_accounts_by_risk(accounts);
    if order.is_empty() {
        return by_risk;
    }

    let mut remaining: std::collections::HashMap<String, AccountDashboard> = accounts
        .iter()
        .map(|acc| (acc.account.id.clone(), acc.clone()))
        .collect();
    let mut ordered = Vec::with_capacity(accounts.len());
    for id in order {
        if let Some(acc) = remaining.remove(id) {
            ordered.push(acc);
        }
    }
    for acc in by_risk {
        if remaining.remove(&acc.account.id).is_some() {
            ordered.push(acc);
        }
    }
    ordered
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaTier {
    pub id: String,
    pub label: String,
    pub utilization: f64,
    pub resets_at: Option<i64>, // UTC epoch milliseconds
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    #[serde(default)]
    pub unlimited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuota {
    pub provider: ProviderKind,
    pub plan: String,
    pub tiers: Vec<QuotaTier>,
    pub queried_at: i64, // UTC epoch milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierForecast {
    pub state: RiskState,
    pub rate_per_hour: f64,
    pub projected_utilization_at_reset: f64,
    pub exhaustion_at: Option<i64>, // UTC epoch milliseconds
    pub sample_count: usize,
    pub observation_minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierDashboard {
    pub quota: QuotaTier,
    pub forecast: TierForecast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub sampled_at: i64,
    pub utilization: f64,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub id: String,
    pub provider: ProviderKind,
    pub display_name: String,
    pub enabled: bool,
    pub credential_source: CredentialSource,
    pub has_credential: bool,
    pub config: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDashboard {
    pub account: PublicAccount,
    pub credential_status: CredentialStatus,
    pub stale: bool,
    pub error: Option<String>,
    pub tiers: Vec<TierDashboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub accounts: Vec<AccountDashboard>,
    pub leading_risk: RiskState,
    pub refreshed_at: i64,
    pub next_refresh_at: i64,
    pub refresh_in_progress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderConfig {
    Claude,
    Codex,
    Gemini,
    Copilot {
        #[serde(rename = "githubDomain")]
        github_domain: Option<String>,
    },
    Kimi,
    Zhipu {
        region: String, // "cn" | "global"
    },
    ZhipuTeam {
        #[serde(rename = "organizationId")]
        organization_id: String,
        #[serde(rename = "projectId")]
        project_id: String,
    },
    Minimax {
        region: String, // "cn" | "global"
    },
    Zenmux {
        #[serde(rename = "quotaUrl")]
        quota_url: String,
    },
    Volcengine {
        region: String,
    },
}

impl ProviderConfig {
    pub fn provider_kind(&self) -> ProviderKind {
        match self {
            ProviderConfig::Claude => ProviderKind::Claude,
            ProviderConfig::Codex => ProviderKind::Codex,
            ProviderConfig::Gemini => ProviderKind::Gemini,
            ProviderConfig::Copilot { .. } => ProviderKind::Copilot,
            ProviderConfig::Kimi => ProviderKind::Kimi,
            ProviderConfig::Zhipu { .. } => ProviderKind::Zhipu,
            ProviderConfig::ZhipuTeam { .. } => ProviderKind::ZhipuTeam,
            ProviderConfig::Minimax { .. } => ProviderKind::Minimax,
            ProviderConfig::Zenmux { .. } => ProviderKind::Zenmux,
            ProviderConfig::Volcengine { .. } => ProviderKind::Volcengine,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub id: Option<String>,
    pub display_name: String,
    pub enabled: bool,
    pub config: ProviderConfig,
    pub secret: Option<SecretPayload>,
    pub remove_credential: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecretPayload {
    ApiKey {
        #[serde(rename = "apiKey")]
        api_key: String,
    },
    Volcengine {
        #[serde(rename = "accessKeyId")]
        access_key_id: String,
        #[serde(rename = "secretAccessKey")]
        secret_access_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceAuthStatus {
    Pending {
        #[serde(rename = "retryAfterSeconds")]
        retry_after_seconds: u64,
    },
    Authorized {
        account: PublicAccount,
    },
    Expired,
    Denied {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProbe {
    pub provider: ProviderKind,
    pub status: CredentialStatus,
    pub message: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub refresh_interval_minutes: i64,
    /// Manual overview card order. Empty means sort by risk severity.
    #[serde(default)]
    pub account_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl AppError {
    pub fn new<C: Into<String>, M: Into<String>>(code: C, message: M, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}
