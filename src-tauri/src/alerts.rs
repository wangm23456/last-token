use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::domain::{
    AlertRule, AppError, CredentialStatus, DashboardSnapshot, RiskState, TierDashboard,
};
use crate::forecast::is_quota_cycle_break;
use crate::storage::Storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertEventKind {
    ThresholdReached,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlertState {
    pub account_id: String,
    pub tier_id: String,
    pub last_resets_at: Option<i64>,
    pub last_utilization: f64,
    pub threshold_notified: bool,
    pub exhausted_notified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlertDecision {
    pub event: Option<AlertEventKind>,
    pub next_state: AlertState,
}

pub fn evaluate_alert(
    rule: &AlertRule,
    previous: Option<&AlertState>,
    tier: &TierDashboard,
    polling_interval_mins: i64,
) -> AlertDecision {
    let utilization = tier.quota.utilization;
    let resets_at = tier.quota.resets_at;

    let mut threshold_notified = previous.map(|p| p.threshold_notified).unwrap_or(false);
    let mut exhausted_notified = previous.map(|p| p.exhausted_notified).unwrap_or(false);

    if let Some(prev) = previous {
        if is_quota_cycle_break(
            prev.last_utilization,
            prev.last_resets_at,
            utilization,
            resets_at,
            polling_interval_mins,
        ) {
            threshold_notified = false;
            exhausted_notified = false;
        }
    }

    let event = if utilization >= 99.9 && !exhausted_notified {
        Some(AlertEventKind::Exhausted)
    } else if utilization >= f64::from(rule.threshold_percent) && !threshold_notified {
        Some(AlertEventKind::ThresholdReached)
    } else {
        None
    };

    // Marking notified flags happens only after a successful send in process_alerts.
    // Here we only propose the next observation + which event should fire.
    AlertDecision {
        event,
        next_state: AlertState {
            account_id: previous
                .map(|p| p.account_id.clone())
                .unwrap_or_default(),
            tier_id: rule.tier_id.clone(),
            last_resets_at: resets_at,
            last_utilization: utilization,
            threshold_notified,
            exhausted_notified,
        },
    }
}

pub fn format_threshold_body(tier: &TierDashboard, threshold: u8) -> String {
    let label = &tier.quota.label;
    let utilization = tier.quota.utilization;
    match tier.forecast.state {
        RiskState::Learning => format!(
            "{label} 已使用 {utilization:.1}%，达到 {threshold}% 告警阈值。消耗速度仍在学习。"
        ),
        RiskState::UnknownReset => format!(
            "{label} 已使用 {utilization:.1}%，达到 {threshold}% 告警阈值。当前周期缺少重置时间，暂无法计算可靠速度。"
        ),
        _ => format!(
            "{label} 已使用 {utilization:.1}%，达到 {threshold}% 告警阈值。当前消耗速度 {:.1}%/时。",
            tier.forecast.rate_per_hour
        ),
    }
}

pub fn format_exhausted_body(tier: &TierDashboard) -> String {
    format!(
        "{} 已被判定为耗尽（当前使用率 {:.1}%），请等待额度重置。",
        tier.quota.label, tier.quota.utilization
    )
}

pub trait NotificationSender: Send + Sync {
    fn send(&self, title: &str, body: &str) -> Result<(), String>;
}

pub struct TauriNotificationSender<'a, R: Runtime> {
    pub app: &'a AppHandle<R>,
}

impl<'a, R: Runtime> NotificationSender for TauriNotificationSender<'a, R> {
    fn send(&self, title: &str, body: &str) -> Result<(), String> {
        self.app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }
}

pub fn process_alerts(
    storage: &Storage,
    sender: &dyn NotificationSender,
    snapshot: &DashboardSnapshot,
    polling_interval_mins: i64,
) -> Result<(), AppError> {
    let mut first_error: Option<AppError> = None;

    for account in &snapshot.accounts {
        if !account.account.enabled
            || account.credential_status != CredentialStatus::Valid
            || account.error.is_some()
            || account.stale
        {
            continue;
        }

        let rules = match storage.list_alert_rules(&account.account.id) {
            Ok(r) => r,
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(AppError::new("database_error", e.to_string(), false));
                }
                continue;
            }
        };

        for rule in rules.into_iter().filter(|r| r.enabled) {
            let Some(tier) = account
                .tiers
                .iter()
                .find(|t| t.quota.id == rule.tier_id && !t.quota.unlimited)
            else {
                continue;
            };

            let previous = match storage.get_alert_state(&account.account.id, &rule.tier_id) {
                Ok(s) => s,
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(AppError::new("database_error", e.to_string(), false));
                    }
                    continue;
                }
            };

            let mut decision = evaluate_alert(&rule, previous.as_ref(), tier, polling_interval_mins);
            decision.next_state.account_id = account.account.id.clone();
            decision.next_state.tier_id = rule.tier_id.clone();

            if let Some(event) = decision.event {
                let (title, body) = match event {
                    AlertEventKind::ThresholdReached => (
                        format!("额度告警 · {}", account.account.display_name),
                        format_threshold_body(tier, rule.threshold_percent),
                    ),
                    AlertEventKind::Exhausted => (
                        format!("额度已耗尽 · {}", account.account.display_name),
                        format_exhausted_body(tier),
                    ),
                };

                match sender.send(&title, &body) {
                    Ok(()) => match event {
                        AlertEventKind::Exhausted => {
                            decision.next_state.threshold_notified = true;
                            decision.next_state.exhausted_notified = true;
                        }
                        AlertEventKind::ThresholdReached => {
                            decision.next_state.threshold_notified = true;
                        }
                    },
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(AppError::new(
                                "notification_error",
                                e,
                                true,
                            ));
                        }
                    }
                }
            }

            if let Err(e) = storage.upsert_alert_state(&decision.next_state) {
                if first_error.is_none() {
                    first_error = Some(AppError::new("database_error", e.to_string(), false));
                }
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub fn request_permission_granted<R: Runtime>(app: &AppHandle<R>) -> Result<bool, AppError> {
    let notifications = app.notification();
    let state = notifications
        .permission_state()
        .map_err(|e| AppError::new("notification_error", e.to_string(), true))?;

    let final_state = match state {
        PermissionState::Granted => PermissionState::Granted,
        PermissionState::Denied => PermissionState::Denied,
        _ => notifications
            .request_permission()
            .map_err(|e| AppError::new("notification_error", e.to_string(), true))?,
    };

    Ok(matches!(final_state, PermissionState::Granted))
}
