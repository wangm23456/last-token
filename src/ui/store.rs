//! `AppStore`: one entity per process holding the data every window shows.
//! Views observe it; async operations funnel through it so all surfaces
//! (main window, tray panel) see the same snapshot.

use std::sync::Arc;

use gpui_kit::*;

use crate::alerts::NotificationSender;
use crate::domain::{
    AccountInput, CredentialProbe, CredentialStatus, DashboardSnapshot, DeviceAuthStatus,
    HistoryPoint, PublicAccount, Settings,
};
use crate::service::{self, Core};

/// Collects alert notifications during a refresh; the refresh owner posts them
/// to the system notification center afterwards. `NotificationSender` requires
/// `Send + Sync`, which GPUI's `AsyncApp` is not.
pub struct ChannelNotifier(pub Arc<parking_lot::Mutex<Vec<(String, String)>>>);

impl NotificationSender for ChannelNotifier {
    fn send(&self, title: &str, body: &str) -> Result<(), String> {
        self.0.lock().push((title.to_string(), body.to_string()));
        Ok(())
    }
}

pub struct AppStore {
    pub core: Arc<Core>,
    pub language: String,
    pub snapshot: Option<DashboardSnapshot>,
    pub settings: Settings,
    pub accounts: Vec<PublicAccount>,
    pub probes: Vec<CredentialProbe>,
    pub history: Option<(String, String, Vec<HistoryPoint>)>,
    pub busy: Busy,
    /// One-shot toast messages; views drain and display them.
    pub toasts: Vec<Toast>,
    pub device_flow: Option<DeviceFlowStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Idle,
    Refreshing,
    Loading,
    Saving,
    Importing,
    Probing,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub error: bool,
}

/// Copilot device-link state driven by `poll_copilot_device_flow`.
#[derive(Debug, Clone)]
pub enum DeviceFlowStatus {
    Requesting,
    Polling {
        device_code: String,
        user_code: String,
        verify_uri: String,
        interval: u64,
        github_domain: Option<String>,
    },
    Failed(String),
}

impl AppStore {
    pub fn new(core: Arc<Core>, cx: &mut Context<Self>) -> Self {
        let language = crate::i18n::detect_language();
        let settings = service::get_settings(&core).unwrap_or(Settings {
            refresh_interval_minutes: 5,
            account_order: Vec::new(),
        });
        let mut store = Self {
            core,
            language,
            snapshot: None,
            settings,
            accounts: Vec::new(),
            probes: Vec::new(),
            history: None,
            busy: Busy::Loading,
            toasts: Vec::new(),
            device_flow: None,
        };
        store.reload_all(cx);
        store
    }

    pub fn t(&self, key: &str) -> String {
        crate::i18n::tr(&self.language, key)
    }

    pub fn tp(&self, key: &str, params: &[(&str, &str)]) -> String {
        crate::i18n::tr_params(&self.language, key, params)
    }

    pub fn toast(&mut self, message: String, error: bool) {
        self.toasts.push(Toast { message, error });
    }

    /// Reload accounts, CLI probes and the dashboard without forcing a
    /// network refresh.
    pub fn reload_all(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        self.busy = Busy::Loading;
        cx.spawn(async move |this, cx| {
            let snapshot = service::get_dashboard_snapshot(&core, false);
            let accounts = service::list_accounts(&core);
            let probes = service::probe_cli_credentials(&core);
            let settings = service::get_settings(&core);
            let _ = this.update(cx, |store, cx| {
                match snapshot {
                    Ok(s) => store.snapshot = Some(s),
                    Err(e) => store.toast(e.message, true),
                }
                if let Ok(a) = accounts {
                    store.accounts = a;
                }
                if let Ok(p) = probes {
                    store.probes = p;
                }
                if let Ok(s) = settings {
                    store.settings = s;
                }
                store.busy = Busy::Idle;
                cx.notify();
            });
        })
        .detach();
    }

    /// Full network refresh of every enabled account.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy == Busy::Refreshing {
            return;
        }
        self.busy = Busy::Refreshing;
        cx.notify();
        let core = self.core.clone();
        let sent = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let sender = Arc::new(ChannelNotifier(sent.clone()));
        cx.spawn(async move |this, cx| {
            let result = core
                .on_tokio(service::refresh_all_action(core.clone(), sender))
                .await;
            let notifications = std::mem::take(&mut *sent.lock());
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(snap) => store.snapshot = Some(snap),
                    Err(e) => store.toast(e.message, true),
                }
                store.busy = Busy::Idle;
                for (title, body) in notifications {
                    cx.show_system_notification(SystemNotification {
                        tag: title.clone().into(),
                        title: title.into(),
                        body: body.into(),
                        actions: Vec::new(),
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn load_history(&mut self, account_id: String, tier_id: String, cx: &mut Context<Self>) {
        let result = service::get_tier_history(&self.core, &account_id, &tier_id, 24);
        match result {
            Ok(points) => self.history = Some((account_id, tier_id, points)),
            Err(e) => self.toast(e.message, true),
        }
        cx.notify();
    }

    pub fn save_account(&mut self, input: AccountInput, cx: &mut Context<Self>) {
        let core = self.core.clone();
        self.busy = Busy::Saving;
        cx.spawn(async move |this, cx| {
            let result = service::save_account(&core, input);
            let _ = this.update(cx, |store, cx| {
                store.busy = Busy::Idle;
                match result {
                    Ok(_) => {
                        store.toast(store.t("providers.toasts.saved"), false);
                        store.reload_all(cx);
                        store.refresh(cx);
                    }
                    Err(e) => store.toast(
                        store.tp(
                            "providers.toasts.saveFailed",
                            &[("message", e.message.as_str())],
                        ),
                        true,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn delete_account(&mut self, account_id: String, cx: &mut Context<Self>) {
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = service::delete_account(&core, &account_id);
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(()) => {
                        store.toast(store.t("providers.toasts.deleted"), false);
                        store.reload_all(cx);
                        store.refresh(cx);
                    }
                    Err(e) => store.toast(
                        store.tp(
                            "providers.toasts.deleteFailed",
                            &[("message", e.message.as_str())],
                        ),
                        true,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn discover_env(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        self.busy = Busy::Importing;
        cx.spawn(async move |this, cx| {
            let result = service::discover_env_accounts(&core);
            let _ = this.update(cx, |store, cx| {
                store.busy = Busy::Idle;
                match result {
                    Ok(created) => {
                        if created.is_empty() {
                            store.toast(store.t("providers.toasts.noNewEnv"), false);
                        } else {
                            let names: Vec<_> =
                                created.iter().map(|a| a.display_name.as_str()).collect();
                            store.toast(
                                store.tp(
                                    "providers.toasts.autoAdded",
                                    &[
                                        ("count", &created.len().to_string()),
                                        ("names", &names.join(", ")),
                                    ],
                                ),
                                false,
                            );
                        }
                        store.reload_all(cx);
                        store.refresh(cx);
                    }
                    Err(e) => store.toast(
                        store.tp(
                            "providers.toasts.discoverFailed",
                            &[("message", e.message.as_str())],
                        ),
                        true,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// One-click import: probe CLI credentials and discover env accounts.
    pub fn one_click_import(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        self.busy = Busy::Importing;
        cx.spawn(async move |this, cx| {
            let probes = service::probe_cli_credentials(&core);
            let env = service::discover_env_accounts(&core);
            let _ = this.update(cx, |store, cx| {
                store.busy = Busy::Idle;
                let mut parts: Vec<String> = Vec::new();
                if let Ok(probes) = probes {
                    let new_cli = probes
                        .iter()
                        .filter(|p| {
                            p.status == CredentialStatus::Valid
                                && store
                                    .accounts
                                    .iter()
                                    .all(|a| Some(&a.id) != p.account_id.as_ref())
                        })
                        .count();
                    if new_cli > 0 {
                        parts.push(store.tp(
                            "providers.toasts.importCliPart",
                            &[("names", &new_cli.to_string())],
                        ));
                    }
                }
                match env {
                    Ok(created) if !created.is_empty() => {
                        let names: Vec<_> =
                            created.iter().map(|a| a.display_name.as_str()).collect();
                        parts.push(store.tp(
                            "providers.toasts.importEnvPart",
                            &[("names", &names.join(", "))],
                        ));
                    }
                    Err(e) => {
                        store.toast(
                            store.tp(
                                "providers.toasts.importFailed",
                                &[("message", e.message.as_str())],
                            ),
                            true,
                        );
                        store.reload_all(cx);
                        cx.notify();
                        return;
                    }
                    _ => {}
                }
                if parts.is_empty() {
                    store.toast(store.t("providers.toasts.importEmpty"), false);
                } else {
                    let joined = parts.join(&store.t("providers.toasts.importJoin"));
                    store.toast(
                        store.tp("providers.toasts.importOk", &[("parts", &joined)]),
                        false,
                    );
                }
                store.reload_all(cx);
                store.refresh(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn update_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = service::update_settings(&core, &settings);
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(()) => {
                        store.settings = settings;
                        store.toast(store.t("settings.toasts.updated"), false);
                        store.refresh(cx);
                    }
                    Err(e) => store.toast(
                        store.tp(
                            "settings.toasts.updateFailed",
                            &[("message", e.message.as_str())],
                        ),
                        true,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn update_account_order(&mut self, order: Vec<String>, cx: &mut Context<Self>) {
        let core = self.core.clone();
        let snapshot = self.snapshot.clone();
        // Optimistic: reorder the visible snapshot immediately.
        if let Some(snap) = &mut self.snapshot {
            snap.accounts = crate::domain::apply_account_order(&snap.accounts, &order);
        }
        self.settings.account_order = order.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service::update_account_order(&core, &order);
            if let Err(e) = result {
                let _ = this.update(cx, |store, cx| {
                    store.snapshot = snapshot;
                    store.toast(e.message, true);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn clear_history(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = service::clear_history(&core);
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(()) => {
                        store.toast(store.t("settings.toasts.cleared"), false);
                        store.refresh(cx);
                    }
                    Err(e) => store.toast(
                        store.tp(
                            "settings.toasts.clearFailed",
                            &[("message", e.message.as_str())],
                        ),
                        true,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_language(&mut self, lang: &str, cx: &mut Context<Self>) {
        self.language = lang.to_string();
        let _ = crate::i18n::save_language(lang);
        cx.notify();
    }

    // ── Copilot device flow ──────────────────────────────────────

    pub fn start_device_flow(&mut self, github_domain: Option<String>, cx: &mut Context<Self>) {
        self.device_flow = Some(DeviceFlowStatus::Requesting);
        cx.notify();
        let core = self.core.clone();
        cx.spawn(async move |this, cx| {
            let result = core
                .on_tokio(service::start_copilot_device_flow(core.clone(), github_domain.clone()))
                .await;
            match result {
                Ok(start) => {
                    let interval = start.interval.max(5);
                    let poll_device = start.device_code.clone();
                    let _ = this.update(cx, |store, cx| {
                        store.device_flow = Some(DeviceFlowStatus::Polling {
                            device_code: start.device_code,
                            user_code: start.user_code,
                            verify_uri: start.verification_uri,
                            interval,
                            github_domain: github_domain.clone(),
                        });
                        cx.notify();
                    });
                    // Poll until authorized / expired / denied.
                    loop {
                        let poll = core
                            .on_tokio(service::poll_copilot_device_flow(
                                core.clone(),
                                poll_device.clone(),
                                github_domain.clone(),
                                Some(interval),
                            ))
                            .await;
                        match poll {
                            Ok(DeviceAuthStatus::Pending { retry_after_seconds }) => {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_secs(retry_after_seconds))
                                    .await;
                            }
                            Ok(DeviceAuthStatus::Authorized { account }) => {
                                let _ = this.update(cx, |store, cx| {
                                    store.device_flow = None;
                                    store.toast(store.t("providers.toasts.linkSuccess"), false);
                                    store.save_account(
                                        AccountInput {
                                            id: Some(account.id),
                                            display_name: account.display_name,
                                            enabled: true,
                                            config: account.config,
                                            secret: None,
                                            remove_credential: None,
                                            alert_rules: account.alert_rules,
                                        },
                                        cx,
                                    );
                                });
                                break;
                            }
                            Ok(DeviceAuthStatus::Expired) => {
                                let _ = this.update(cx, |store, cx| {
                                    store.device_flow = Some(DeviceFlowStatus::Failed(
                                        store.t("providers.toasts.codeExpired"),
                                    ));
                                    cx.notify();
                                });
                                break;
                            }
                            Ok(DeviceAuthStatus::Denied { message }) => {
                                let _ = this.update(cx, |store, cx| {
                                    store.device_flow = Some(DeviceFlowStatus::Failed(message));
                                    cx.notify();
                                });
                                break;
                            }
                            Err(e) => {
                                let _ = this.update(cx, |store, cx| {
                                    store.device_flow = Some(DeviceFlowStatus::Failed(e.message));
                                    cx.notify();
                                });
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = this.update(cx, |store, cx| {
                        store.device_flow = Some(DeviceFlowStatus::Failed(e.message));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn cancel_device_flow(&mut self, cx: &mut Context<Self>) {
        self.device_flow = None;
        cx.notify();
    }
}
