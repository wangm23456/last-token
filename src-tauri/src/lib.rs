pub mod domain;
pub mod storage;
pub mod providers;
pub mod forecast;
pub mod alerts;
pub mod tray;
pub mod file_secret_store;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use parking_lot::Mutex;
use crate::domain::{
    apply_account_order, validate_alert_rules, AccountDashboard, AccountInput, AppError, DashboardSnapshot, DeviceAuthStatus,
    ProviderConfig, ProviderKind, PublicAccount, RiskState, SecretPayload, Settings,
    TierDashboard, CredentialProbe, CredentialStatus, CredentialSource, QuotaTier,
};
use crate::storage::{Storage, SecretStore};
use crate::file_secret_store::FileSecretStore;
use crate::providers::query_provider_quota;
use crate::forecast::compute_forecast;

// ── AppState Definition ──────────────────────────────────────────

pub struct AppState {
    pub db: Arc<Storage>,
    pub secret_store: Arc<dyn SecretStore>,
    pub client: reqwest::Client,
    pub gemini_token_cache: Mutex<HashMap<String, (String, i64)>>,
    pub tray_state: Mutex<Option<crate::tray::TrayState>>,
    pub refresh_lock: tokio::sync::Mutex<()>,
    pub refresh_in_progress: std::sync::atomic::AtomicBool,
    /// Per-account last refresh failure message, cleared on the next
    /// successful refresh. Surfaced through AccountDashboard.error so the
    /// UI can show query failures (not just credential issues).
    pub refresh_failures: Mutex<HashMap<String, String>>,
    /// Boring wake primitive for the background scheduler: `update_settings`
    /// calls `notify_one` so the in-flight `tokio::time::sleep` is cut short
    /// and the new interval takes effect on the next wake.
    pub scheduler_wake: Arc<tokio::sync::Notify>,
}

// ── Background Scheduler / Refresh Task ──────────────────────────

pub async fn refresh_all_action(app: &AppHandle) -> Result<DashboardSnapshot, AppError> {
    let state = app.state::<AppState>();
    
    // Acquire batch lock or return early if refresh in progress
    let _lock = match state.refresh_lock.try_lock() {
        Ok(guard) => {
            state.refresh_in_progress.store(true, std::sync::atomic::Ordering::SeqCst);
            guard
        }
        Err(_) => {
            // Already in progress, fetch current view with refreshInProgress = true
            let snapshot = get_dashboard_snapshot(app, true)?;
            return Ok(snapshot);
        }
    };

    // Emit refresh status change to UI
    if let Ok(snap) = get_dashboard_snapshot(app, true) {
        let _ = crate::tray::update_tray_menu(app, &snap);
        let _ = app.emit("quota-updated", &snap);
    }

    let accounts_raw: Vec<(String, String, String, bool, String, String)> = state.db.list_accounts_raw().map_err(|e| {
        state.refresh_in_progress.store(false, std::sync::atomic::Ordering::SeqCst);
        AppError::new("database_error", e.to_string(), false)
    })?;

    // Filter to only enabled accounts
    let enabled_accounts: Vec<(String, String, String, bool, String, String)> = accounts_raw
        .into_iter()
        .filter(|row| row.3)
        .collect();

    // Query in parallel
    use futures::StreamExt;
    let mut stream = futures::stream::iter(enabled_accounts.into_iter())
        .map(|row| {
            let id = row.0;
            let provider_str = row.1;
            let config_json = row.5;
            let state_ref = state.clone();
            async move {
                let provider = match serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\"")) {
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
                    state_ref.secret_store.as_ref(),
                    &id,
                    &state_ref.client,
                    &state_ref.gemini_token_cache,
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
                // Save snapshots
                for tier in quota.tiers {
                    let _ = state.db.insert_snapshot(
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
                // Successful refresh — drop any prior failure so the
                // dashboard returns to a clean state.
                state.refresh_failures.lock().remove(&id);
            }
            Err(e) => {
                // Persist the failure so the dashboard can surface it.
                // Transient errors stay until the next success; deterministic
                // ones are also persisted and cleared the same way. The
                // stale-grace rule will hide the prior values if the failure
                // window grows beyond the cutoff.
                eprintln!("Refresh failed for account {id}: {e}");
                state.refresh_failures.lock().insert(id.clone(), e);
            }
        }
    }

    // Prune history (older than 30 days) once a day
    let thirty_days_ms = 30 * 24 * 60 * 60 * 1000;
    let prune_threshold = now_ms - thirty_days_ms;
    let _ = state.db.prune_snapshots(prune_threshold);

    state.refresh_in_progress.store(false, std::sync::atomic::Ordering::SeqCst);
    
    // Get updated snapshot
    let snapshot = get_dashboard_snapshot(app, false)?;

    let polling_interval_mins = state
        .db
        .get_settings()
        .map(|s| s.refresh_interval_minutes)
        .unwrap_or(5);
    let sender = crate::alerts::TauriNotificationSender { app };
    if let Err(e) = crate::alerts::process_alerts(
        state.db.as_ref(),
        &sender,
        &snapshot,
        polling_interval_mins,
    ) {
        eprintln!("process_alerts failed: {e}");
    }

    // Update tray and notify UI
    let _ = crate::tray::update_tray_menu(app, &snapshot);
    let _ = app.emit("quota-updated", &snapshot);

    Ok(snapshot)
}

fn get_dashboard_snapshot(app: &AppHandle, force_in_progress: bool) -> Result<DashboardSnapshot, AppError> {
    let state = app.state::<AppState>();
    
    let accounts_raw = state.db.list_accounts_raw().map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let settings = state.db.get_settings().map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    let mut accounts = Vec::new();
    let mut leading_risk = RiskState::Safe;

    for (id, provider_str, display_name, enabled, source_str, config_json) in accounts_raw {
        if !enabled {
            continue;
        }

        let provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
            .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
        let source = parse_credential_source(&source_str);
        let config = serde_json::from_str::<ProviderConfig>(&config_json)
            .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

        // Determine credential status and has_credential
        let (has_credential, credential_status, err_msg) = if source == CredentialSource::CliAuto {
            let creds = match provider {
                ProviderKind::Claude => crate::providers::read_claude_cli_credentials(),
                ProviderKind::Codex => crate::providers::read_codex_cli_credentials(),
                ProviderKind::Gemini => crate::providers::read_gemini_cli_credentials(),
                _ => crate::providers::CliCredentials {
                    token: None,
                    refresh_token: None,
                    account_id: None,
                    status: CredentialStatus::NotFound,
                    message: None,
                },
            };
            (creds.token.is_some(), creds.status, creds.message)
        } else {
            let has_cred = match provider {
                ProviderKind::Volcengine => {
                    let ak = state.secret_store.get_secret(&id, "access_key_id").ok().flatten();
                    let sk = state.secret_store.get_secret(&id, "secret_access_key").ok().flatten();
                    ak.is_some() && sk.is_some()
                }
                ProviderKind::Copilot => {
                    state.secret_store.get_secret(&id, "github_token").ok().flatten().is_some()
                }
                _ => {
                    state.secret_store.get_secret(&id, "api_key").ok().flatten().is_some()
                }
            };
            let status = if has_cred { CredentialStatus::Valid } else { CredentialStatus::NotFound };
            if !has_cred {
                // If a user-created/discovered account has no credential present in shell/env,
                // do not show it as a failed configuration on the dashboard.
                continue;
            }
            (has_cred, status, None)
        };

        let alert_rules = state.db.list_alert_rules(&id).unwrap_or_default();
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

        // Determine if account status is stale / has query error
        // Fetch snapshots for each tier
        let mut tiers = Vec::new();
        let mut stale = false;

        // Stale grace cutoff: a refresh interval of 5/10/15/30 minutes means
        // a sample stops being meaningful after a few missed refreshes.
        // max(10 minutes, 2 * refresh_interval) is the window where a
        // previously-good value is still shown with stale=true. Beyond that
        // we drop the tier entirely so the dashboard does not display
        // arbitrarily old quota values.
        let refresh_interval_ms = settings.refresh_interval_minutes * 60 * 1000;
        let stale_threshold_ms = std::cmp::max(10 * 60 * 1000, 2 * refresh_interval_ms);

        // Only tiers from the latest successful batch are shown: one refresh
        // writes every tier with a shared `sampled_at`, so this keeps every
        // window the upstream currently returns and drops tiers that only
        // exist in older responses. Stable display order: 5h < weekly/model
        // < monthly < unknown.
        let mut tier_ids = state.db.get_current_tier_ids(&id).map_err(|e| {
            AppError::new("database_error", e.to_string(), false)
        })?;
        tier_ids.sort_by_key(|t| crate::domain::tier_sort_key(t));

        for tier_id in &tier_ids {
            let latest = match state.db.get_latest_snapshot(&id, tier_id) {
                Ok(Some(l)) => l,
                Ok(None) => continue,
                Err(e) => return Err(AppError::new("database_error", e.to_string(), false)),
            };
            let history = state.db.get_snapshots(&id, tier_id.as_str(), 100).unwrap_or_default();

            let age = now_ms - latest.sampled_at;
            // Past the grace cutoff the value is no longer trustworthy;
            // mark the account stale and skip the tier instead of showing
            // an arbitrary old number.
            if age > stale_threshold_ms {
                stale = true;
                continue;
            }

            if credential_status != CredentialStatus::Valid {
                continue;
            }

            // unlimited comes from the upstream response verbatim; a 0%
            // finite quota is NOT unlimited and still goes through the
            // normal learning/safe path.
            let forecast = compute_forecast(
                latest.unlimited,
                latest.utilization,
                latest.resets_at,
                &history,
                now_ms,
                settings.refresh_interval_minutes,
            );

            if crate::domain::risk_severity(forecast.state) > crate::domain::risk_severity(leading_risk) {
                leading_risk = forecast.state;
            }

            tiers.push(TierDashboard {
                quota: QuotaTier {
                    id: tier_id.to_string(),
                    label: match tier_id.as_str() {
                        "five_hour" => "5-Hour Session".to_string(),
                        "seven_day" => "7-Day Limit".to_string(),
                        "weekly_limit" => "Weekly Limit".to_string(),
                        "monthly" => "Monthly Limit".to_string(),
                        "30_day" => "30-Day Limit".to_string(),
                        "gemini_pro" => "Gemini Pro".to_string(),
                        "gemini_flash" => "Gemini Flash".to_string(),
                        "gemini_flash_lite" => "Gemini Flash Lite".to_string(),
                        _ => tier_id.to_string(),
                    },
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

        // The latest refresh failure (if any) takes priority over the
        // credential-level message so the UI can show "query failed" vs.
        // "no credential" without having to inspect two fields.
        let query_error = state.refresh_failures.lock().get(&id).cloned();
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

    let refresh_in_progress = force_in_progress || state.refresh_in_progress.load(std::sync::atomic::Ordering::SeqCst);

    let accounts = apply_account_order(&accounts, &settings.account_order);

    Ok(DashboardSnapshot {
        accounts,
        leading_risk,
        refreshed_at: now_ms,
        next_refresh_at: now_ms + settings.refresh_interval_minutes * 60 * 1000,
        refresh_in_progress,
    })
}

// ── Background Scheduling Loop ───────────────────────────────────

fn start_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // The loop always re-reads `refresh_interval_minutes` from settings
        // before sleeping, so `update_settings` just needs to wake us. The
        // `Notify` primitive is shared through `AppState`; the `select!`
        // arms let the in-flight `sleep` resolve early and pick up the
        // new interval on the next iteration.
        let wake = app.state::<AppState>().scheduler_wake.clone();
        loop {
            let interval_mins = app
                .state::<AppState>()
                .db
                .get_settings()
                .map(|s| s.refresh_interval_minutes)
                .unwrap_or(5);

            let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(
                interval_mins as u64 * 60,
            ));
            tokio::pin!(sleep);
            tokio::select! {
                _ = sleep.as_mut() => {}
                _ = wake.notified() => {}
            }

            let _ = refresh_all_action(&app).await;
        }
    });
}

// ── Tauri Commands ───────────────────────────────────────────────

#[tauri::command]
async fn get_dashboard(app: AppHandle) -> Result<DashboardSnapshot, AppError> {
    get_dashboard_snapshot(&app, false)
}

#[tauri::command]
async fn get_tier_history(
    app: AppHandle,
    account_id: String,
    tier_id: String,
    hours: i32,
) -> Result<Vec<crate::domain::HistoryPoint>, AppError> {
    let state = app.state::<AppState>();
    let cl_hours = hours.clamp(1, 720);
    
    // Get snapshots for the account + tier
    let history = state.db.get_snapshots(&account_id, &tier_id, 500).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    // Filter to last N hours
    let now_ms = chrono::Utc::now().timestamp_millis();
    let threshold = now_ms - (cl_hours as i64 * 60 * 60 * 1000);
    
    let filtered = history
        .into_iter()
        .filter(|pt| pt.sampled_at >= threshold)
        .collect();

    Ok(filtered)
}

#[tauri::command]
async fn refresh_all(app: AppHandle) -> Result<DashboardSnapshot, AppError> {
    refresh_all_action(&app).await
}

#[tauri::command]
async fn open_main_window(app: AppHandle) -> Result<(), AppError> {
    crate::tray::show_main_window(&app);
    Ok(())
}

fn parse_credential_source(s: &str) -> CredentialSource {
    match s {
        "cli_auto" => CredentialSource::CliAuto,
        _ => CredentialSource::Env, // default to env for legacy/unknown sources
    }
}

#[tauri::command]
async fn list_accounts(app: AppHandle) -> Result<Vec<PublicAccount>, AppError> {
    let state = app.state::<AppState>();
    let raw = state.db.list_accounts_raw().map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    let mut list = Vec::new();
    for (id, provider_str, display_name, enabled, source_str, config_json) in raw {
        let provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
            .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
        let source = parse_credential_source(&source_str);
        let config = serde_json::from_str::<ProviderConfig>(&config_json)
            .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

        let has_credential = if source == CredentialSource::CliAuto {
            match provider {
                ProviderKind::Claude => crate::providers::read_claude_cli_credentials().token.is_some(),
                ProviderKind::Codex => crate::providers::read_codex_cli_credentials().token.is_some(),
                ProviderKind::Gemini => crate::providers::read_gemini_cli_credentials().token.is_some(),
                _ => false,
            }
        } else {
            match provider {
                ProviderKind::Volcengine => {
                    let ak = state.secret_store.get_secret(&id, "access_key_id").ok().flatten();
                    let sk = state.secret_store.get_secret(&id, "secret_access_key").ok().flatten();
                    ak.is_some() && sk.is_some()
                }
                ProviderKind::Copilot => {
                    state.secret_store.get_secret(&id, "github_token").ok().flatten().is_some()
                }
                _ => {
                    state.secret_store.get_secret(&id, "api_key").ok().flatten().is_some()
                }
            }
        };

        let alert_rules = state.db.list_alert_rules(&id).unwrap_or_default();
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

#[tauri::command]
async fn save_account(app: AppHandle, input: AccountInput) -> Result<PublicAccount, AppError> {
    let state = app.state::<AppState>();
    
    // Reject secret together with removeCredential=true
    if input.remove_credential == Some(true) && input.secret.is_some() {
        return Err(AppError::new("validation_error", "Cannot set secret and removeCredential=true simultaneously", false));
    }

    let is_new = input.id.is_none();
    let account_id = match &input.id {
        Some(id) => id.clone(),
        None => {
            // Generate id based on provider
            let uuid = uuid::Uuid::new_v4().to_string();
            format!("{}:{}", input.config.provider_kind().provider_label(), uuid)
        }
    };

    // Reject provider change for an existing account
    if !is_new {
        if let Some((_, provider_str, _, _, _, _)) = state.db.get_account_raw(&account_id).map_err(|e| AppError::new("database_error", e.to_string(), false))? {
            let existing_provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
                .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;
            if existing_provider != input.config.provider_kind() {
                return Err(AppError::new("validation_error", "Provider type cannot be changed for an existing account", false));
            }
        }
    }

    // In-memory backup mechanism to restore in case SQLite save fails
    let mut backup_secrets = HashMap::new();
    let keys_to_backup = match input.config.provider_kind() {
        ProviderKind::Volcengine => vec!["access_key_id", "secret_access_key"],
        ProviderKind::Copilot => vec!["github_token"],
        _ => vec!["api_key"],
    };

    for key in &keys_to_backup {
        if let Ok(Some(val)) = state.secret_store.get_secret(&account_id, key) {
            backup_secrets.insert(key.to_string(), val);
        }
    }

    // 1. Update Secrets
    let mut status = CredentialStatus::Valid;

    if let Some(true) = input.remove_credential {
        // Delete credentials
        for key in &keys_to_backup {
            let _ = state.secret_store.delete_secret(&account_id, key);
        }
        status = CredentialStatus::NotFound;
    } else if let Some(secret_payload) = &input.secret {
        match secret_payload {
            SecretPayload::ApiKey { api_key } => {
                if let Err(e) = state.secret_store.set_secret(&account_id, "api_key", api_key) {
                    return Err(AppError::new("secret_store_error", format!("Failed to save API key: {e}"), false));
                }
            }
            SecretPayload::Volcengine { access_key_id, secret_access_key } => {
                if let Err(e) = state.secret_store.set_secret(&account_id, "access_key_id", access_key_id) {
                    return Err(AppError::new("secret_store_error", format!("Failed to save access key: {e}"), false));
                }
                if let Err(e) = state.secret_store.set_secret(&account_id, "secret_access_key", secret_access_key) {
                    // Rollback access_key_id
                    if let Some(bk) = backup_secrets.get("access_key_id") {
                        let _ = state.secret_store.set_secret(&account_id, "access_key_id", bk);
                    } else {
                        let _ = state.secret_store.delete_secret(&account_id, "access_key_id");
                    }
                    return Err(AppError::new("secret_store_error", format!("Failed to save secret key: {e}"), false));
                }
            }
        }
    } else {
        // No secret supplied; check whether the required env variable is present
        // so the UI's has_credential reflects reality instead of defaulting to Valid.
        let has_cred = match input.config.provider_kind() {
            ProviderKind::Volcengine => {
                let ak = state.secret_store.get_secret(&account_id, "access_key_id").ok().flatten();
                let sk = state.secret_store.get_secret(&account_id, "secret_access_key").ok().flatten();
                ak.is_some() && sk.is_some()
            }
            ProviderKind::Copilot => state.secret_store.get_secret(&account_id, "github_token").ok().flatten().is_some(),
            _ => state.secret_store.get_secret(&account_id, "api_key").ok().flatten().is_some(),
        };
        status = if has_cred { CredentialStatus::Valid } else { CredentialStatus::NotFound };
    }

    // 2. Save Metadata in SQLite
    let provider_str = input.config.provider_kind().to_string();
    let source_str = if account_id.starts_with("cli:") { "cli_auto" } else { "env" };
    let config_json = serde_json::to_string(&input.config).unwrap();

    validate_alert_rules(&input.alert_rules)?;

    let sqlite_res = state.db.save_account(
        &account_id,
        &provider_str,
        &input.display_name,
        input.enabled,
        source_str,
        &config_json,
        Some(input.alert_rules.as_slice()),
    );

    if let Err(db_err) = sqlite_res {
        // Rollback secrets in store
        for key in &keys_to_backup {
            if let Some(bk) = backup_secrets.get(*key) {
                let _ = state.secret_store.set_secret(&account_id, key, bk);
            } else {
                let _ = state.secret_store.delete_secret(&account_id, key);
            }
        }
        return Err(AppError::new("database_error", format!("Failed to save account in SQLite: {db_err}"), false));
    }

    // If enabled & has secrets, schedule non-blocking refresh
    if input.enabled {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = refresh_all_action(&app_clone).await;
        });
    }

    let alert_rules = state.db.list_alert_rules(&account_id).unwrap_or_default();
    let public_acc = PublicAccount {
        id: account_id,
        provider: input.config.provider_kind(),
        display_name: input.display_name,
        enabled: input.enabled,
        credential_source: if source_str == "cli_auto" { CredentialSource::CliAuto } else { CredentialSource::Env },
        has_credential: status == CredentialStatus::Valid,
        config: input.config,
        alert_rules,
    };

    if let Ok(snap) = get_dashboard_snapshot(&app, false) {
        let _ = app.emit("quota-updated", &snap);
    }

    Ok(public_acc)
}

#[tauri::command]
async fn delete_account(app: AppHandle, account_id: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();

    // Reject deletion of cli_auto accounts
    if account_id.starts_with("cli:") {
        return Err(AppError::new("validation_error", "CLI-auto accounts cannot be deleted. Disabling is recommended.", false));
    }

    let account_raw = state.db.get_account_raw(&account_id).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    let (_, provider_str, _, _, _, _) = match account_raw {
        Some(a) => a,
        None => return Err(AppError::new("not_found", "Account not found", false)),
    };

    let provider = serde_json::from_str::<ProviderKind>(&format!("\"{provider_str}\""))
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

    // Delete secrets
    let keys_to_delete = match provider {
        ProviderKind::Volcengine => vec!["access_key_id", "secret_access_key"],
        ProviderKind::Copilot => vec!["github_token"],
        _ => vec!["api_key"],
    };

    for key in keys_to_delete {
        let _ = state.secret_store.delete_secret(&account_id, key);
    }

    // Delete from SQLite (cascades quota snapshots)
    state.db.delete_account(&account_id).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    // Immediately remove from tray and notify
    if let Ok(snap) = get_dashboard_snapshot(&app, false) {
        let _ = crate::tray::update_tray_menu(&app, &snap);
        let _ = app.emit("quota-updated", &snap);
    }

    Ok(())
}

#[tauri::command]
async fn discover_env_accounts(app: AppHandle) -> Result<Vec<PublicAccount>, AppError> {
    let state = app.state::<AppState>();
    let existing = list_accounts(app.clone()).await?;
    let existing_providers: std::collections::HashSet<_> = existing.iter().map(|a| a.provider).collect();

    let candidates: Vec<(ProviderKind, &'static str, ProviderConfig, Vec<&'static str>)> = vec![
        (ProviderKind::Claude, "Claude (Env)", ProviderConfig::Claude, vec!["api_key"]),
        (ProviderKind::Codex, "Codex (Env)", ProviderConfig::Codex, vec!["api_key"]),
        (ProviderKind::Gemini, "Gemini (Env)", ProviderConfig::Gemini, vec!["api_key"]),
        (ProviderKind::Kimi, "Kimi (Env)", ProviderConfig::Kimi, vec!["api_key"]),
        (ProviderKind::Zhipu, "Zhipu (Env)", ProviderConfig::Zhipu { region: "cn".to_string() }, vec!["api_key"]),
        (ProviderKind::Minimax, "Minimax (Env)", ProviderConfig::Minimax { region: "cn".to_string() }, vec!["api_key"]),
        (ProviderKind::Copilot, "Copilot (Env)", ProviderConfig::Copilot { github_domain: None }, vec!["github_token"]),
        (ProviderKind::Volcengine, "Volcengine (Env)", ProviderConfig::Volcengine { region: "cn-beijing".to_string() }, vec!["access_key_id", "secret_access_key"]),
    ];

    let mut created = Vec::new();
    for (provider, display_name, config, secret_names) in candidates {
        if existing_providers.contains(&provider) {
            continue;
        }
        let provider_label = provider.provider_label();
        let dummy_id = format!("{}:env-discovery", provider_label);
        let all_present = secret_names.iter().all(|secret_name| {
            state.secret_store.get_secret(&dummy_id, secret_name).ok().flatten().is_some()
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
        if let Ok(acc) = save_account(app.clone(), input).await {
            created.push(acc);
        }
    }

    if !created.is_empty() {
        if let Ok(snap) = get_dashboard_snapshot(&app, false) {
            let _ = app.emit("quota-updated", &snap);
        }
    }

    Ok(created)
}

#[tauri::command]
async fn probe_cli_credentials(app: AppHandle) -> Result<Vec<CredentialProbe>, AppError> {
    let mut probes = Vec::new();

    // Claude
    let claude = crate::providers::read_claude_cli_credentials();
    probes.push(CredentialProbe {
        provider: ProviderKind::Claude,
        status: claude.status,
        message: claude.message,
        account_id: Some("cli:claude".to_string()),
    });

    // Codex
    let codex = crate::providers::read_codex_cli_credentials();
    probes.push(CredentialProbe {
        provider: ProviderKind::Codex,
        status: codex.status,
        message: codex.message,
        account_id: Some("cli:codex".to_string()),
    });

    // Gemini
    let gemini = crate::providers::read_gemini_cli_credentials();
    probes.push(CredentialProbe {
        provider: ProviderKind::Gemini,
        status: gemini.status,
        message: gemini.message,
        account_id: Some("cli:gemini".to_string()),
    });

    // If valid, upsert official account metadata in SQLite
    let state = app.state::<AppState>();
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

            // Only insert if not exists to preserve custom display names / enabled settings
            if state.db.get_account_raw(id).ok().flatten().is_none() {
                let display_name = format!("{} (CLI)", probe.provider.provider_label());
                let _ = state.db.save_account(
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceFlowStartResult {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[tauri::command]
async fn start_copilot_device_flow(
    app: AppHandle,
    github_domain: Option<String>,
) -> Result<DeviceFlowStartResult, AppError> {
    let state = app.state::<AppState>();
    let domain = github_domain.as_deref().unwrap_or("github.com");

    let client = &state.client;
    let url = format!("https://{domain}/login/device/code");

    let client_id = if domain == "github.com" {
        "Iv1.b507a08c87ecfe98"
    } else {
        "Ov23li8tweQw6odWQebz"
    };

    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("scope", "read:user"),
        ])
        .send()
        .await
        .map_err(|e| AppError::new("network_error", e.to_string(), true))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::new("api_error", format!("GitHub device flow start failed (HTTP {status}): {text}"), false));
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

#[tauri::command]
async fn poll_copilot_device_flow(
    app: AppHandle,
    device_code: String,
    github_domain: Option<String>,
    current_interval: Option<u64>,
) -> Result<DeviceAuthStatus, AppError> {
    let state = app.state::<AppState>();
    let domain = github_domain.as_deref().unwrap_or("github.com");

    let client = &state.client;
    let url = format!("https://{domain}/login/oauth/access_token");

    let client_id = if domain == "github.com" {
        "Iv1.b507a08c87ecfe98"
    } else {
        "Ov23li8tweQw6odWQebz"
    };

    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", &device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::new("network_error", e.to_string(), true))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::new("api_error", format!("GitHub token poll failed (HTTP {status}): {text}"), false));
    }

    let val: GitHubTokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::new("parse_error", e.to_string(), false))?;

    if let Some(err) = val.error {
        match err.as_str() {
            "authorization_pending" => {
                return Ok(DeviceAuthStatus::Pending { retry_after_seconds: current_interval.unwrap_or(5) });
            }
            "slow_down" => {
                // GitHub requires the polling interval to be increased by at
                // least 5 seconds; bump from the last-known interval if any.
                let next = current_interval.unwrap_or(5) + 5;
                return Ok(DeviceAuthStatus::Pending { retry_after_seconds: next });
            }
            "expired_token" => {
                return Ok(DeviceAuthStatus::Expired);
            }
            "access_denied" => {
                return Ok(DeviceAuthStatus::Denied {
                    message: val.error_description.unwrap_or_else(|| "Access denied".to_string()),
                });
            }
            _ => {
                return Ok(DeviceAuthStatus::Denied {
                    message: val
                        .error_description
                        .unwrap_or_else(|| format!("Unknown GitHub error: {err}")),
                });
            }
        }
    }

    let token = val.access_token.ok_or_else(|| {
        AppError::new("api_error", "Missing access_token in GitHub response", false)
    })?;

    // Success! Resolve user login name
    let api_base = if domain == "github.com" {
        "https://api.github.com".to_string()
    } else {
        format!("https://{domain}/api/v3")
    };

    let user_resp = client
        .get(&format!("{api_base}/user"))
        .header("Authorization", format!("token {token}"))
        .header("User-Agent", "GitHubCopilotChat/0.38.2")
        .send()
        .await
        .map_err(|e| AppError::new("network_error", format!("Failed to retrieve GitHub user: {e}"), true))?;

    if !user_resp.status().is_success() {
        return Err(AppError::new("api_error", "Failed to retrieve GitHub user info", false));
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

    // Create public account representation
    let account_id = if domain == "github.com" {
        user_id.to_string()
    } else {
        format!("{}:{}", domain, user_id)
    };

    let display_name = format!("Copilot ({login})");
    let config = ProviderConfig::Copilot {
        github_domain: github_domain.clone(),
    };

    // Save token in memory store (falls back to env at runtime)
    if let Err(e) = state.secret_store.set_secret(&account_id, "github_token", &token) {
        return Err(AppError::new("secret_store_error", format!("Failed to save Copilot token: {e}"), false));
    }

    // Save in SQLite
    let provider_str = "copilot";
    let config_json = serde_json::to_string(&config).unwrap();
    state.db.save_account(&account_id, provider_str, &display_name, true, "env", &config_json, None).map_err(|e| {
        let _ = state.secret_store.delete_secret(&account_id, "github_token");
        AppError::new("database_error", e.to_string(), false)
    })?;

    // Schedule non-blocking refresh
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = refresh_all_action(&app_clone).await;
    });

    let alert_rules = state.db.list_alert_rules(&account_id).unwrap_or_default();
    let public_acc = PublicAccount {
        id: account_id,
        provider: ProviderKind::Copilot,
        display_name,
        enabled: true,
        credential_source: CredentialSource::Env,
        has_credential: true,
        config,
        alert_rules,
    };

    if let Ok(snap) = get_dashboard_snapshot(&app, false) {
        let _ = app.emit("quota-updated", &snap);
    }

    Ok(DeviceAuthStatus::Authorized { account: public_acc })
}

#[tauri::command]
async fn get_settings(app: AppHandle) -> Result<Settings, AppError> {
    let state = app.state::<AppState>();
    state.db.get_settings().map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })
}

#[tauri::command]
async fn update_settings(app: AppHandle, input: Settings) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    
    // Reject values outside the literal set {5,10,15,30}
    if ![5, 10, 15, 30].contains(&input.refresh_interval_minutes) {
        return Err(AppError::new("validation_error", "Interval must be 5, 10, 15, or 30 minutes", false));
    }

    state.db.update_settings(input.refresh_interval_minutes).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;
    // Persist overview card order when provided via settings update.
    state.db.update_account_order(&input.account_order).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;
    // settings changes wake the scheduler so the next sleep is
    // re-evaluated against the new interval instead of waiting out the
    // old one, and we kick off a refresh so the new cadence is observed
    // immediately.
    state.scheduler_wake.notify_one();
    let _ = refresh_all_action(&app).await;

    Ok(())
}

#[tauri::command]
async fn update_account_order(app: AppHandle, order: Vec<String>) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    state.db.update_account_order(&order).map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;
    // Rebuild native tray menu immediately so right-click order matches overview.
    if let Ok(snap) = get_dashboard_snapshot(&app, false) {
        let _ = crate::tray::update_tray_menu(&app, &snap);
        let _ = app.emit("quota-updated", &snap);
    }
    Ok(())
}

#[tauri::command]
async fn clear_history(app: AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    state.db.clear_history().map_err(|e| {
        AppError::new("database_error", e.to_string(), false)
    })?;

    // clear_history() recomputes forecasts from no samples
    let _ = refresh_all_action(&app).await;

    Ok(())
}

// ── ProviderKind / Config Label Helpers ──────────────────────────

impl ProviderKind {
    pub fn provider_label(&self) -> &'static str {
        match self {
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
        }
    }
}

// ── Run Entry Point ──────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[tauri::command]
async fn request_notification_permission(app: AppHandle) -> Result<bool, AppError> {
    crate::alerts::request_permission_granted(&app)
}


pub fn run() {
    // CrabNebula DevTools — debug builds only, captures setup/IPC/panic logs
    // so silent failures (e.g. main window not surfacing) are diagnosable.
    // See https://tauri.app/develop/debug/crabnebula-devtools/
    #[cfg(debug_assertions)]
    let devtools = tauri_plugin_devtools::init();

    let mut builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.plugin(tauri_plugin_notification::init());
    builder = builder.plugin(tauri_plugin_widgets::init());
    builder
        .setup(|app| {
            // Menu-bar app: no Dock icon, lives in the system tray.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Resolve app_data_dir and create db
            let app_data_dir = app.path().app_data_dir().expect("Failed to get AppData directory");
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create AppData directory");
            let db_path = app_data_dir.join("last-token.sqlite3");

            let db = Arc::new(Storage::new(db_path).expect("Failed to initialize database"));
            let secret_store = Arc::new(FileSecretStore::new(app_data_dir.clone()));
            let client = reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .expect("Failed to build reqwest client");

            let app_state = AppState {
                db,
                secret_store,
                client,
                gemini_token_cache: Mutex::new(HashMap::new()),
                tray_state: Mutex::new(None),
                refresh_lock: tokio::sync::Mutex::new(()),
                refresh_in_progress: std::sync::atomic::AtomicBool::new(false),
                refresh_failures: Mutex::new(HashMap::new()),
                scheduler_wake: Arc::new(tokio::sync::Notify::new()),
            };
            app.manage(app_state);

            // Create Tray Icon
            crate::tray::create_tray(app.handle())?;

            // Probe CLI credentials at startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = probe_cli_credentials(app_handle.clone()).await;
                
                // Initial dashboard update to sync state
                let _ = refresh_all_action(&app_handle).await;
            });

            // Start background scheduler
            start_scheduler(app.handle().clone());

            // A direct app launch should surface the configuration window. Calling
            // set_focus() through the shared helper also prompts an initially hidden
            // WKWebView to paint after show() on macOS.
            crate::tray::show_main_window(app.handle());
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Never destroy surfaces: closing hides so the tray can
                // reopen both the main window and the panel later.
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Focused(false) => {
                // Clicking outside the tray panel dismisses it.
                if window.label() == crate::tray::TRAY_PANEL_LABEL {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            get_tier_history,
            refresh_all,
            open_main_window,
            list_accounts,
            save_account,
            delete_account,
            discover_env_accounts,
            probe_cli_credentials,
            start_copilot_device_flow,
            poll_copilot_device_flow,
            get_settings,
            update_settings,
            update_account_order,
            clear_history,
            request_notification_permission
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
