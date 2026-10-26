//! Application core: state, persistence wiring, and every operation the UI
//! performs. Framework-free except for the notification seam, so it stays
//! testable outside GPUI.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::alerts::{process_alerts, NotificationSender};
use crate::domain::{
    apply_account_order, validate_alert_rules, AccountDashboard, AccountInput, AppError,
    CredentialProbe, CredentialSource, CredentialStatus, DashboardSnapshot, DeviceAuthStatus,
    HistoryPoint, ProviderConfig, ProviderKind, PublicAccount, QuotaTier, RiskState, SecretPayload,
    Settings, TierDashboard,
};
use crate::file_secret_store::FileSecretStore;
use crate::forecast::compute_forecast;
use crate::providers::{query_provider_quota, CliCredentials};
use crate::storage::{SecretStore, Storage};

/// Root path for the SQLite database, secret store and language file.
pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.wangmeng.last-token")
}

pub struct Core {
    pub db: Arc<Storage>,
    pub secret_store: Arc<dyn SecretStore>,
    pub client: reqwest::Client,
    pub tokio: tokio::runtime::Runtime,
    pub gemini_token_cache: Mutex<HashMap<String, (String, i64)>>,
    /// tokio Mutex: its guard is Send and stays held across awaits in the
    /// refresh task. parking_lot's is not.
    pub refresh_lock: tokio::sync::Mutex<()>,
    pub refresh_in_progress: AtomicBool,
    /// Per-account last refresh failure message, cleared on the next
    /// successful refresh. Surfaced through `AccountDashboard.error`.
    pub refresh_failures: Mutex<HashMap<String, String>>,
    /// Wake primitive for the background scheduler: `update_settings` calls
    /// `notify_one` so the in-flight sleep is cut short and the new interval
    /// takes effect on the next wake.
    pub scheduler_wake: Arc<tokio::sync::Notify>,
}

impl Core {
    pub fn new() -> Result<Arc<Self>, AppError> {
        Self::new_in(app_data_dir())
    }

    /// Create the core rooted at a specific data directory. Tests point this
    /// at a temp dir so they never touch the user's real database.
    pub fn new_in(dir: PathBuf) -> Result<Arc<Self>, AppError> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::new("io_error", e.to_string(), false))?;
        let db = Storage::new(dir.join("last-token.sqlite3"))
            .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| AppError::new("network_error", e.to_string(), true))?;
        let tokio = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| AppError::new("runtime_error", e.to_string(), false))?;
        Ok(Arc::new(Self {
            db: Arc::new(db),
            secret_store: Arc::new(FileSecretStore::new(dir)),
            client,
            tokio,
            gemini_token_cache: Mutex::new(HashMap::new()),
            refresh_lock: tokio::sync::Mutex::new(()),
            refresh_in_progress: AtomicBool::new(false),
            refresh_failures: Mutex::new(HashMap::new()),
            scheduler_wake: Arc::new(tokio::sync::Notify::new()),
        }))
    }

    /// Run `fut` on the app's tokio runtime. reqwest needs a tokio reactor
    /// context; GPUI's own executors do not provide one.
    pub async fn on_tokio<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.tokio.spawn(fut).await.expect("tokio task panicked")
    }
}

// ── Dashboard ────────────────────────────────────────────────────

fn parse_credential_source(s: &str) -> CredentialSource {
    match s {
        "cli_auto" => CredentialSource::CliAuto,
        _ => CredentialSource::Env,
    }
}

fn tier_label(tier_id: &str) -> String {
    match tier_id {
        "five_hour" => "5-Hour Session".to_string(),
        "seven_day" => "7-Day Limit".to_string(),
        "weekly_limit" => "Weekly Limit".to_string(),
        "monthly" => "Monthly Limit".to_string(),
        "30_day" => "30-Day Limit".to_string(),
        "gemini_pro" => "Gemini Pro".to_string(),
        "gemini_flash" => "Gemini Flash".to_string(),
        "gemini_flash_lite" => "Gemini Flash Lite".to_string(),
        _ => tier_id.to_string(),
    }
}

fn has_stored_credential(
    core: &Core,
    provider: ProviderKind,
    id: &str,
) -> bool {
    match provider {
        ProviderKind::Volcengine => {
            let ak = core.secret_store.get_secret(id, "access_key_id").ok().flatten();
            let sk = core.secret_store.get_secret(id, "secret_access_key").ok().flatten();
            ak.is_some() && sk.is_some()
        }
        ProviderKind::Copilot => core
            .secret_store
            .get_secret(id, "github_token")
            .ok()
            .flatten()
            .is_some(),
        _ => core
            .secret_store
            .get_secret(id, "api_key")
            .ok()
            .flatten()
            .is_some(),
    }
}

fn cli_credentials(provider: ProviderKind) -> CliCredentials {
    match provider {
        ProviderKind::Claude => crate::providers::read_claude_cli_credentials(),
        ProviderKind::Codex => crate::providers::read_codex_cli_credentials(),
        ProviderKind::Gemini => crate::providers::read_gemini_cli_credentials(),
        _ => CliCredentials {
            token: None,
            refresh_token: None,
            account_id: None,
            status: CredentialStatus::NotFound,
            message: None,
        },
    }
}

fn parse_account_row(
    row: (String, String, String, bool, String, String),
) -> Result<(String, ProviderKind, String, bool, CredentialSource, ProviderConfig), AppError> {
    let (id, provider_str, display_name, enabled, source_str, config_json) = row;
    let provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
    let config = serde_json::from_str::<ProviderConfig>(&config_json)
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
    Ok((
        id,
        provider,
        display_name,
        enabled,
        parse_credential_source(&source_str),
        config,
    ))
}

pub fn get_dashboard_snapshot(
    core: &Arc<Core>,
    force_in_progress: bool,
) -> Result<DashboardSnapshot, AppError> {
    let accounts_raw = core
        .db
        .list_accounts_raw()
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let settings = core
        .db
        .get_settings()
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;

    let mut accounts = Vec::new();
    let mut leading_risk = RiskState::Safe;

    for row in accounts_raw {
        let (id, provider, display_name, enabled, source, config) = parse_account_row(row)?;
        if !enabled {
            continue;
        }

        let (has_credential, credential_status, err_msg) = if source == CredentialSource::CliAuto {
            let creds = cli_credentials(provider);
            (creds.token.is_some(), creds.status, creds.message)
        } else {
            let has_cred = has_stored_credential(core, provider, &id);
            if !has_cred {
                // A user-created account without a credential is not a failed
                // configuration; keep it off the dashboard.
                continue;
            }
            (true, CredentialStatus::Valid, None)
        };

        let alert_rules = core.db.list_alert_rules(&id).unwrap_or_default();
        let public_acc = PublicAccount {
            id: id.clone(),
            provider,
            display_name,
            enabled,
            credential_source: source,
            has_credential,
            config,
            alert_rules,
        };

        // Stale grace cutoff: a refresh interval of 5/10/15/30 minutes means a
        // sample stops being meaningful after a few missed refreshes.
        // max(10 minutes, 2 * refresh_interval) is the window where a
        // previously-good value is still shown with stale=true. Beyond that we
        // drop the tier entirely.
        let refresh_interval_ms = settings.refresh_interval_minutes * 60 * 1000;
        let stale_threshold_ms = std::cmp::max(10 * 60 * 1000, 2 * refresh_interval_ms);

        let mut tier_ids = core
            .db
            .get_current_tier_ids(&id)
            .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
        tier_ids.sort_by_key(|t| crate::domain::tier_sort_key(t));

        let mut tiers = Vec::new();
        let mut stale = false;

        for tier_id in &tier_ids {
            let latest = match core.db.get_latest_snapshot(&id, tier_id) {
                Ok(Some(l)) => l,
                Ok(None) => continue,
                Err(e) => return Err(AppError::new("database_error", e.to_string(), false)),
            };
            let history = core.db.get_snapshots(&id, tier_id.as_str(), 100).unwrap_or_default();

            let age = now_ms - latest.sampled_at;
            if age > stale_threshold_ms {
                stale = true;
                continue;
            }

            if credential_status != CredentialStatus::Valid {
                continue;
            }

            // unlimited comes from the upstream response verbatim; a 0% finite
            // quota is NOT unlimited.
            let forecast = compute_forecast(
                latest.unlimited,
                latest.utilization,
                latest.resets_at,
                &history,
                now_ms,
                settings.refresh_interval_minutes,
            );

            if crate::domain::risk_severity(forecast.state)
                > crate::domain::risk_severity(leading_risk)
            {
                leading_risk = forecast.state;
            }

            tiers.push(TierDashboard {
                quota: QuotaTier {
                    id: tier_id.to_string(),
                    label: tier_label(tier_id),
                    utilization: latest.utilization,
                    resets_at: latest.resets_at,
                    used: latest.used,
                    limit: latest.limit_value,
                    unit: latest.unit,
                    unlimited: latest.unlimited,
                },
                forecast,
            });
        }

        // The latest refresh failure takes priority over the credential-level
        // message so the UI can show "query failed" vs. "no credential".
        let query_error = core.refresh_failures.lock().get(&id).cloned();
        let effective_status = if query_error.is_some() {
            CredentialStatus::Unavailable
        } else {
            credential_status
        };
        let combined_error = query_error.or(err_msg);

        accounts.push(AccountDashboard {
            account: public_acc,
            credential_status: effective_status,
            stale,
            error: combined_error,
            tiers,
        });
    }

    let refresh_in_progress =
        force_in_progress || core.refresh_in_progress.load(Ordering::SeqCst);
    let accounts = apply_account_order(&accounts, &settings.account_order);

    Ok(DashboardSnapshot {
        accounts,
        leading_risk,
        refreshed_at: now_ms,
        next_refresh_at: now_ms + settings.refresh_interval_minutes * 60 * 1000,
        refresh_in_progress,
    })
}

// ── Refresh ──────────────────────────────────────────────────────

/// Full refresh cycle. Must run on the tokio runtime (reqwest); call through
/// [`Core::on_tokio`]. `sender` receives alert notifications.
pub async fn refresh_all_action(
    core: Arc<Core>,
    sender: Arc<dyn NotificationSender>,
) -> Result<DashboardSnapshot, AppError> {
    let _lock = match core.refresh_lock.try_lock() {
        Ok(guard) => {
            core.refresh_in_progress.store(true, Ordering::SeqCst);
            guard
        }
        Err(_) => {
            // Already in progress; return the current view flagged as busy.
            return get_dashboard_snapshot(&core, true);
        }
    };

    let accounts_raw = core
        .db
        .list_accounts_raw()
        .map_err(|e| {
            core.refresh_in_progress.store(false, Ordering::SeqCst);
            AppError::new("database_error", e.to_string(), false)
        })?;

    let enabled_accounts: Vec<_> = accounts_raw.into_iter().filter(|row| row.3).collect();

    use futures::StreamExt;
    let mut stream = futures::stream::iter(enabled_accounts)
        .map(|row| {
            let (id, provider_str, _, _, _, config_json) = row;
            let core = core.clone();
            async move {
                let provider =
                    match serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\"")) {
                        Ok(p) => p,
                        Err(e) => return (id, Err(e.to_string())),
                    };
                let config = match serde_json::from_str::<ProviderConfig>(&config_json) {
                    Ok(c) => c,
                    Err(e) => return (id, Err(e.to_string())),
                };
                let res = query_provider_quota(
                    provider,
                    &config,
                    core.secret_store.as_ref(),
                    &id,
                    &core.client,
                    &core.gemini_token_cache,
                )
                .await;
                (id, res)
            }
        })
        .buffer_unordered(4);

    let now_ms = chrono::Utc::now().timestamp_millis();

    while let Some((id, result)) = stream.next().await {
        match result {
            Ok(quota) => {
                for tier in quota.tiers {
                    let _ = core.db.insert_snapshot(
                        &id,
                        &tier.id,
                        tier.utilization,
                        tier.resets_at,
                        now_ms,
                        tier.used,
                        tier.limit,
                        tier.unit.as_deref(),
                        tier.unlimited,
                    );
                }
                core.refresh_failures.lock().remove(&id);
            }
            Err(e) => {
                eprintln!("Refresh failed for account {id}: {e}");
                core.refresh_failures.lock().insert(id, e);
            }
        }
    }

    // Prune history older than 30 days on every cycle; the delete is cheap.
    let thirty_days_ms = 30 * 24 * 60 * 60 * 1000;
    let _ = core.db.prune_snapshots(now_ms - thirty_days_ms);

    core.refresh_in_progress.store(false, Ordering::SeqCst);

    let snapshot = get_dashboard_snapshot(&core, false)?;

    let polling_interval_mins = core
        .db
        .get_settings()
        .map(|s| s.refresh_interval_minutes)
        .unwrap_or(5);
    if let Err(e) = process_alerts(core.db.as_ref(), sender.as_ref(), &snapshot, polling_interval_mins) {
        eprintln!("process_alerts failed: {e}");
    }

    Ok(snapshot)
}

/// Background scheduler: refresh on the configured interval until dropped.
pub fn start_scheduler(core: Arc<Core>, sender: Arc<dyn NotificationSender>) {
    let core_task = core.clone();
    core.tokio.spawn(async move {
        loop {
            let interval_mins = core_task
                .db
                .get_settings()
                .map(|s| s.refresh_interval_minutes)
                .unwrap_or(5);

            let sleep =
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_mins as u64 * 60));
            tokio::pin!(sleep);
            tokio::select! {
                _ = sleep.as_mut() => {}
                _ = core_task.scheduler_wake.notified() => {}
            }

            let _ = refresh_all_action(core_task.clone(), sender.clone()).await;
        }
    });
}

// ── History ──────────────────────────────────────────────────────

pub fn get_tier_history(
    core: &Arc<Core>,
    account_id: &str,
    tier_id: &str,
    hours: i32,
) -> Result<Vec<HistoryPoint>, AppError> {
    let cl_hours = hours.clamp(1, 720);
    let history = core
        .db
        .get_snapshots(account_id, tier_id, 500)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let threshold = now_ms - (cl_hours as i64 * 60 * 60 * 1000);
    Ok(history
        .into_iter()
        .filter(|pt| pt.sampled_at >= threshold)
        .collect())
}

// ── Accounts ─────────────────────────────────────────────────────

pub fn list_accounts(core: &Arc<Core>) -> Result<Vec<PublicAccount>, AppError> {
    let raw = core
        .db
        .list_accounts_raw()
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;

    let mut list = Vec::new();
    for row in raw {
        let (id, provider, display_name, enabled, source, config) = parse_account_row(row)?;
        let has_credential = if source == CredentialSource::CliAuto {
            cli_credentials(provider).token.is_some()
        } else {
            has_stored_credential(core, provider, &id)
        };
        let alert_rules = core.db.list_alert_rules(&id).unwrap_or_default();
        list.push(PublicAccount {
            id,
            provider,
            display_name,
            enabled,
            credential_source: source,
            has_credential,
            config,
            alert_rules,
        });
    }
    Ok(list)
}

fn secret_keys(provider: ProviderKind) -> Vec<&'static str> {
    match provider {
        ProviderKind::Volcengine => vec!["access_key_id", "secret_access_key"],
        ProviderKind::Copilot => vec!["github_token"],
        _ => vec!["api_key"],
    }
}

pub fn save_account(core: &Arc<Core>, input: AccountInput) -> Result<PublicAccount, AppError> {
    if input.remove_credential == Some(true) && input.secret.is_some() {
        return Err(AppError::new(
            "validation_error",
            "Cannot set secret and removeCredential=true simultaneously",
            false,
        ));
    }

    let is_new = input.id.is_none();
    let account_id = match &input.id {
        Some(id) => id.clone(),
        None => {
            let uuid = uuid::Uuid::new_v4().to_string();
            format!("{}:{}", input.config.provider_kind().provider_label(), uuid)
        }
    };

    // Reject provider change for an existing account.
    if !is_new {
        if let Some((_, provider_str, _, _, _, _)) = core
            .db
            .get_account_raw(&account_id)
            .map_err(|e| AppError::new("database_error", e.to_string(), false))?
        {
            let existing_provider =
                serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
                    .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
            if existing_provider != input.config.provider_kind() {
                return Err(AppError::new(
                    "validation_error",
                    "Provider type cannot be changed for an existing account",
                    false,
                ));
            }
        }
    }

    // Backup existing secrets so a failed SQLite save can roll back.
    let keys_to_backup = secret_keys(input.config.provider_kind());
    let mut backup_secrets = HashMap::new();
    for key in &keys_to_backup {
        if let Ok(Some(val)) = core.secret_store.get_secret(&account_id, key) {
            backup_secrets.insert(key.to_string(), val);
        }
    }

    let mut status = CredentialStatus::Valid;

    if let Some(true) = input.remove_credential {
        for key in &keys_to_backup {
            let _ = core.secret_store.delete_secret(&account_id, key);
        }
        status = CredentialStatus::NotFound;
    } else if let Some(secret_payload) = &input.secret {
        match secret_payload {
            SecretPayload::ApiKey { api_key } => {
                core.secret_store
                    .set_secret(&account_id, "api_key", api_key)
                    .map_err(|e| {
                        AppError::new("secret_store_error", format!("Failed to save API key: {e}"), false)
                    })?;
            }
            SecretPayload::Volcengine {
                access_key_id,
                secret_access_key,
            } => {
                core.secret_store
                    .set_secret(&account_id, "access_key_id", access_key_id)
                    .map_err(|e| {
                        AppError::new(
                            "secret_store_error",
                            format!("Failed to save access key: {e}"),
                            false,
                        )
                    })?;
                if let Err(e) = core
                    .secret_store
                    .set_secret(&account_id, "secret_access_key", secret_access_key)
                {
                    if let Some(bk) = backup_secrets.get("access_key_id") {
                        let _ = core.secret_store.set_secret(&account_id, "access_key_id", bk);
                    } else {
                        let _ = core.secret_store.delete_secret(&account_id, "access_key_id");
                    }
                    return Err(AppError::new(
                        "secret_store_error",
                        format!("Failed to save secret key: {e}"),
                        false,
                    ));
                }
            }
        }
    } else {
        // No secret supplied: reflect reality instead of defaulting to Valid.
        status = if has_stored_credential(core, input.config.provider_kind(), &account_id) {
            CredentialStatus::Valid
        } else {
            CredentialStatus::NotFound
        };
    }

    let provider_str = input.config.provider_kind().to_string();
    let source_str = if account_id.starts_with("cli:") {
        "cli_auto"
    } else {
        "env"
    };
    let config_json = serde_json::to_string(&input.config).unwrap();

    validate_alert_rules(&input.alert_rules)?;

    let sqlite_res = core.db.save_account(
        &account_id,
        &provider_str,
        &input.display_name,
        input.enabled,
        source_str,
        &config_json,
        Some(input.alert_rules.as_slice()),
    );

    if let Err(db_err) = sqlite_res {
        for key in &keys_to_backup {
            if let Some(bk) = backup_secrets.get(*key) {
                let _ = core.secret_store.set_secret(&account_id, key, bk);
            } else {
                let _ = core.secret_store.delete_secret(&account_id, key);
            }
        }
        return Err(AppError::new(
            "database_error",
            format!("Failed to save account in SQLite: {db_err}"),
            false,
        ));
    }

    let alert_rules = core.db.list_alert_rules(&account_id).unwrap_or_default();
    Ok(PublicAccount {
        id: account_id,
        provider: input.config.provider_kind(),
        display_name: input.display_name,
        enabled: input.enabled,
        credential_source: if source_str == "cli_auto" {
            CredentialSource::CliAuto
        } else {
            CredentialSource::Env
        },
        has_credential: status == CredentialStatus::Valid,
        config: input.config,
        alert_rules,
    })
}

pub fn delete_account(core: &Arc<Core>, account_id: &str) -> Result<(), AppError> {
    if account_id.starts_with("cli:") {
        return Err(AppError::new(
            "validation_error",
            "CLI-auto accounts cannot be deleted. Disabling is recommended.",
            false,
        ));
    }

    let account_raw = core
        .db
        .get_account_raw(account_id)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    let (_, provider_str, _, _, _, _) =
        account_raw.ok_or_else(|| AppError::new("not_found", "Account not found", false))?;
    let provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

    for key in secret_keys(provider) {
        let _ = core.secret_store.delete_secret(account_id, key);
    }
    core.db
        .delete_account(account_id)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    Ok(())
}

pub fn discover_env_accounts(core: &Arc<Core>) -> Result<Vec<PublicAccount>, AppError> {
    let existing = list_accounts(core)?;
    let existing_providers: std::collections::HashSet<_> =
        existing.iter().map(|a| a.provider).collect();

    let candidates: Vec<(ProviderKind, &'static str, ProviderConfig, Vec<&'static str>)> = vec![
        (ProviderKind::Claude, "Claude (Env)", ProviderConfig::Claude, vec!["api_key"]),
        (ProviderKind::Codex, "Codex (Env)", ProviderConfig::Codex, vec!["api_key"]),
        (ProviderKind::Gemini, "Gemini (Env)", ProviderConfig::Gemini, vec!["api_key"]),
        (ProviderKind::Kimi, "Kimi (Env)", ProviderConfig::Kimi, vec!["api_key"]),
        (
            ProviderKind::Zhipu,
            "Zhipu (Env)",
            ProviderConfig::Zhipu { region: "cn".to_string() },
            vec!["api_key"],
        ),
        (
            ProviderKind::Minimax,
            "Minimax (Env)",
            ProviderConfig::Minimax { region: "cn".to_string() },
            vec!["api_key"],
        ),
        (
            ProviderKind::Copilot,
            "Copilot (Env)",
            ProviderConfig::Copilot { github_domain: None },
            vec!["github_token"],
        ),
        (
            ProviderKind::Volcengine,
            "Volcengine (Env)",
            ProviderConfig::Volcengine { region: "cn-beijing".to_string() },
            vec!["access_key_id", "secret_access_key"],
        ),
    ];

    let mut created = Vec::new();
    for (provider, display_name, config, secret_names) in candidates {
        if existing_providers.contains(&provider) {
            continue;
        }
        let provider_label = provider.provider_label();
        let dummy_id = format!("{provider_label}:env-discovery");
        let all_present = secret_names.iter().all(|secret_name| {
            core.secret_store
                .get_secret(&dummy_id, secret_name)
                .ok()
                .flatten()
                .is_some()
        });
        if !all_present {
            continue;
        }
        let input = AccountInput {
            id: None,
            display_name: display_name.to_string(),
            enabled: true,
            config,
            secret: None,
            remove_credential: None,
            alert_rules: vec![],
        };
        if let Ok(acc) = save_account(core, input) {
            created.push(acc);
        }
    }

    Ok(created)
}

// ── CLI probes ───────────────────────────────────────────────────

pub fn probe_cli_credentials(core: &Arc<Core>) -> Result<Vec<CredentialProbe>, AppError> {
    let mut probes = Vec::new();
    for (provider, account_id) in [
        (ProviderKind::Claude, "cli:claude"),
        (ProviderKind::Codex, "cli:codex"),
        (ProviderKind::Gemini, "cli:gemini"),
    ] {
        let creds = cli_credentials(provider);
        probes.push(CredentialProbe {
            provider,
            status: creds.status,
            message: creds.message,
            account_id: Some(account_id.to_string()),
        });
    }

    // If valid, upsert official account metadata. Only insert if absent so
    // custom display names / enabled settings survive.
    for probe in &probes {
        if probe.status == CredentialStatus::Valid {
            let id = probe.account_id.as_ref().unwrap();
            let provider_str = probe.provider.to_string();
            let config_json = match probe.provider {
                ProviderKind::Claude => serde_json::to_string(&ProviderConfig::Claude).unwrap(),
                ProviderKind::Codex => serde_json::to_string(&ProviderConfig::Codex).unwrap(),
                ProviderKind::Gemini => serde_json::to_string(&ProviderConfig::Gemini).unwrap(),
                _ => "".to_string(),
            };
            if core.db.get_account_raw(id).ok().flatten().is_none() {
                let display_name = format!("{} (CLI)", probe.provider.provider_label());
                let _ = core.db.save_account(
                    id,
                    &provider_str,
                    &display_name,
                    true,
                    "cli_auto",
                    &config_json,
                    None,
                );
            }
        }
    }

    Ok(probes)
}

// ── Copilot device flow ──────────────────────────────────────────

#[derive(serde::Deserialize)]
struct GitHubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(serde::Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceFlowStartResult {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

fn copilot_client_id(domain: &str) -> &'static str {
    if domain == "github.com" {
        "Iv1.b507a08c87ecfe98"
    } else {
        "Ov23li8tweQw6odWQebz"
    }
}

/// Must run on the tokio runtime.
pub async fn start_copilot_device_flow(
    core: Arc<Core>,
    github_domain: Option<String>,
) -> Result<DeviceFlowStartResult, AppError> {
    let domain = github_domain.as_deref().unwrap_or("github.com");
    let url = format!("https://{domain}/login/device/code");
    let client_id = copilot_client_id(domain);

    let resp = core
        .client
        .post(&url)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "read:user")])
        .send()
        .await
        .map_err(|e| AppError::new("network_error", e.to_string(), true))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::new(
            "api_error",
            format!("GitHub device flow start failed (HTTP {status}): {text}"),
            false,
        ));
    }

    let val: GitHubDeviceCodeResponse = resp
        .json()
        .await
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

    Ok(DeviceFlowStartResult {
        device_code: val.device_code,
        user_code: val.user_code,
        verification_uri: val.verification_uri,
        expires_in: val.expires_in,
        interval: val.interval,
    })
}

/// Must run on the tokio runtime.
pub async fn poll_copilot_device_flow(
    core: Arc<Core>,
    device_code: String,
    github_domain: Option<String>,
    current_interval: Option<u64>,
) -> Result<DeviceAuthStatus, AppError> {
    let domain = github_domain.as_deref().unwrap_or("github.com");
    let url = format!("https://{domain}/login/oauth/access_token");
    let client_id = copilot_client_id(domain);

    let resp = core
        .client
        .post(&url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::new("network_error", e.to_string(), true))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::new(
            "api_error",
            format!("GitHub token poll failed (HTTP {status}): {text}"),
            false,
        ));
    }

    let val: GitHubTokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

    if let Some(err) = val.error {
        return match err.as_str() {
            "authorization_pending" => Ok(DeviceAuthStatus::Pending {
                retry_after_seconds: current_interval.unwrap_or(5),
            }),
            // GitHub requires the polling interval to grow by at least 5s.
            "slow_down" => Ok(DeviceAuthStatus::Pending {
                retry_after_seconds: current_interval.unwrap_or(5) + 5,
            }),
            "expired_token" => Ok(DeviceAuthStatus::Expired),
            "access_denied" => Ok(DeviceAuthStatus::Denied {
                message: val
                    .error_description
                    .unwrap_or_else(|| "Access denied".to_string()),
            }),
            _ => Ok(DeviceAuthStatus::Denied {
                message: val
                    .error_description
                    .unwrap_or_else(|| format!("Unknown GitHub error: {err}")),
            }),
        };
    }

    let token = val.access_token.ok_or_else(|| {
        AppError::new("api_error", "Missing access_token in GitHub response", false)
    })?;

    let api_base = if domain == "github.com" {
        "https://api.github.com".to_string()
    } else {
        format!("https://{domain}/api/v3")
    };

    let user_resp = core
        .client
        .get(format!("{api_base}/user"))
        .header("Authorization", format!("token {token}"))
        .header("User-Agent", "GitHubCopilotChat/0.38.2")
        .send()
        .await
        .map_err(|e| {
            AppError::new("network_error", format!("Failed to retrieve GitHub user: {e}"), true)
        })?;

    if !user_resp.status().is_success() {
        return Err(AppError::new(
            "api_error",
            "Failed to retrieve GitHub user info",
            false,
        ));
    }

    let user_json: serde_json::Value = user_resp
        .json()
        .await
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
    let login = user_json
        .get("login")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::new("parse_error", "login field missing", false))?;
    let user_id = user_json
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::new("parse_error", "id field missing", false))?;

    let account_id = if domain == "github.com" {
        user_id.to_string()
    } else {
        format!("{domain}:{user_id}")
    };
    let display_name = format!("Copilot ({login})");
    let config = ProviderConfig::Copilot {
        github_domain: github_domain.clone(),
    };

    core.secret_store
        .set_secret(&account_id, "github_token", &token)
        .map_err(|e| {
            AppError::new("secret_store_error", format!("Failed to save Copilot token: {e}"), false)
        })?;

    let config_json = serde_json::to_string(&config).unwrap();
    core.db
        .save_account(&account_id, "copilot", &display_name, true, "env", &config_json, None)
        .map_err(|e| {
            let _ = core.secret_store.delete_secret(&account_id, "github_token");
            AppError::new("database_error", e.to_string(), false)
        })?;

    let alert_rules = core.db.list_alert_rules(&account_id).unwrap_or_default();
    Ok(DeviceAuthStatus::Authorized {
        account: PublicAccount {
            id: account_id,
            provider: ProviderKind::Copilot,
            display_name,
            enabled: true,
            credential_source: CredentialSource::Env,
            has_credential: true,
            config,
            alert_rules,
        },
    })
}

// ── Settings ─────────────────────────────────────────────────────

pub fn get_settings(core: &Arc<Core>) -> Result<Settings, AppError> {
    core.db
        .get_settings()
        .map_err(|e| AppError::new("database_error", e.to_string(), false))
}

pub fn update_settings(core: &Arc<Core>, input: &Settings) -> Result<(), AppError> {
    if ![5, 10, 15, 30].contains(&input.refresh_interval_minutes) {
        return Err(AppError::new(
            "validation_error",
            "Interval must be 5, 10, 15, or 30 minutes",
            false,
        ));
    }
    core.db
        .update_settings(input.refresh_interval_minutes)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    core.db
        .update_account_order(&input.account_order)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))?;
    // Wake the scheduler so the next sleep uses the new interval.
    core.scheduler_wake.notify_one();
    Ok(())
}

pub fn update_account_order(core: &Arc<Core>, order: &[String]) -> Result<(), AppError> {
    core.db
        .update_account_order(order)
        .map_err(|e| AppError::new("database_error", e.to_string(), false))
}

pub fn clear_history(core: &Arc<Core>) -> Result<(), AppError> {
    core.db
        .clear_history()
        .map_err(|e| AppError::new("database_error", e.to_string(), false))
}
