//! Visual preview: opens the real views with mock data on the real platform
//! renderer, captures each window to a PNG, and exits.
//!
//! `cargo run --features preview --bin preview` → `/tmp/lt-ui-*.png`

use std::time::Duration;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Context, Entity, IntoElement, Render, Window, WindowBounds, WindowOptions,
    px, size,
};

use crate::domain::{
    AccountDashboard, CredentialSource, CredentialStatus, DashboardSnapshot, ProviderConfig,
    ProviderKind, PublicAccount, QuotaTier, RiskState, Settings, TierDashboard, TierForecast,
};
use crate::ui::shell::{AppShell, TabPage};
use crate::ui::store::{AppStore, Busy};
use crate::ui::tray::StoreGlobal;
use crate::ui::tray_panel::TrayPanelView;

fn tier(
    now: i64,
    id: &str,
    label: &str,
    utilization: f64,
    state: RiskState,
    resets_in_h: i64,
) -> TierDashboard {
    TierDashboard {
        quota: QuotaTier {
            id: id.into(),
            label: label.into(),
            utilization,
            resets_at: Some(now + resets_in_h * 3_600_000),
            used: Some(utilization * 100.0),
            limit: Some(10_000.0),
            unit: Some(" cr".into()),
            unlimited: false,
        },
        forecast: TierForecast {
            state,
            rate_per_hour: 3.5,
            projected_utilization_at_reset: utilization + 9.0,
            exhaustion_at: (state == RiskState::AtRisk).then_some(now + 12 * 3_600_000),
            sample_count: 42,
            observation_minutes: 300,
        },
    }
}

fn account(id: &str, name: &str, provider: ProviderKind, tiers: Vec<TierDashboard>) -> AccountDashboard {
    AccountDashboard {
        account: PublicAccount {
            id: id.into(),
            provider,
            display_name: name.into(),
            enabled: true,
            credential_source: CredentialSource::Env,
            has_credential: true,
            config: ProviderConfig::Kimi,
            alert_rules: vec![],
        },
        credential_status: CredentialStatus::Valid,
        stale: false,
        error: None,
        tiers,
    }
}

fn mock_snapshot() -> DashboardSnapshot {
    // Real "now" so countdowns render like production.
    let now = chrono::Utc::now().timestamp_millis();
    let safe = account(
        "a:1",
        "Claude 工作号",
        ProviderKind::Claude,
        vec![
            tier(now, "five_hour", "5-Hour Session", 24.0, RiskState::Safe, 3),
            tier(now, "seven_day", "7-Day Limit", 41.0, RiskState::Safe, 72),
        ],
    );
    let risk = account(
        "a:2",
        "Kimi 长期号",
        ProviderKind::Kimi,
        vec![tier(now, "monthly", "Monthly Limit", 82.0, RiskState::AtRisk, 240)],
    );
    let mut broken = account("a:3", "Copilot 团队", ProviderKind::Copilot, vec![]);
    broken.credential_status = CredentialStatus::Unavailable;
    broken.error = Some("token has expired or is invalid".into());

    DashboardSnapshot {
        accounts: vec![safe, risk, broken],
        leading_risk: RiskState::AtRisk,
        refreshed_at: now - 90_000,
        next_refresh_at: now + 210_000,
        refresh_in_progress: false,
    }
}

fn build_store(cx: &mut gpui_kit::App) -> Entity<AppStore> {
    // Preview fixture: isolated data dir so it never touches the user's DB.
    let dir = std::env::temp_dir().join(format!("lt-preview-{}", std::process::id()));
    let core = crate::service::Core::new_in(dir).expect("core");
    let snapshot = mock_snapshot();
    let store = AppStore {
        core,
        language: "zh-CN".into(),
        accounts: snapshot.accounts.iter().map(|a| a.account.clone()).collect(),
        snapshot: Some(snapshot),
        settings: Settings {
            refresh_interval_minutes: 5,
            account_order: vec![],
        },
        probes: vec![
            crate::domain::CredentialProbe {
                provider: ProviderKind::Claude,
                status: CredentialStatus::Valid,
                message: None,
                account_id: Some("cli:claude".into()),
            },
            crate::domain::CredentialProbe {
                provider: ProviderKind::Codex,
                status: CredentialStatus::Expired,
                message: Some("credentials expired".into()),
                account_id: Some("cli:codex".into()),
            },
            crate::domain::CredentialProbe {
                provider: ProviderKind::Gemini,
                status: CredentialStatus::NotFound,
                message: None,
                account_id: Some("cli:gemini".into()),
            },
        ],
        history: None,
        busy: Busy::Idle,
        toasts: vec![],
        device_flow: None,
    };
    cx.new(|_| store)
}

/// Wrapper that swaps the shell page before the first capture.
struct PageFix {
    inner: Entity<AppShell>,
    page: TabPage,
}

impl Render for PageFix {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.inner.update(cx, |shell, _| shell.page = self.page);
        self.inner.clone()
    }
}

pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            cx.set_app_identity("com.wangmeng.last-token", "Last Token Preview");
            gpui_kit::init(cx);
            gpui_kit::component::theme::Theme::change(
                gpui_kit::component::theme::ThemeMode::Dark,
                None,
                cx,
            );

            let store = build_store(cx);
            cx.set_global(StoreGlobal(store.clone()));

            let opts = |w: f32, h: f32| WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(w), px(h)),
                ))),
                ..Default::default()
            };

            let (overview, _) = gpui_kit::open_window(opts(760., 720.), cx, |window, cx| {
                let shell = cx.new(|cx| AppShell::new(window, cx, store.clone()));
                cx.new(|cx| PageFix {
                    inner: shell,
                    page: TabPage::Overview,
                })
            })
            .expect("overview window");

            let (providers, _) = gpui_kit::open_window(opts(760., 720.), cx, |window, cx| {
                let shell = cx.new(|cx| AppShell::new(window, cx, store.clone()));
                cx.new(|cx| PageFix {
                    inner: shell,
                    page: TabPage::Providers,
                })
            })
            .expect("providers window");

            let (settings, _) = gpui_kit::open_window(opts(760., 560.), cx, |window, cx| {
                let shell = cx.new(|cx| AppShell::new(window, cx, store.clone()));
                cx.new(|cx| PageFix {
                    inner: shell,
                    page: TabPage::Settings,
                })
            })
            .expect("settings window");

            let (tray, _) = gpui_kit::open_window(opts(254., 520.), cx, |_, cx| {
                cx.new(|cx| TrayPanelView::new(store.clone(), cx))
            })
            .expect("tray window");

            let captures = [
                (overview, "overview"),
                (providers, "providers"),
                (settings, "settings"),
                (tray, "tray"),
            ];

            cx.spawn(async move |cx| {
                // Let frames land before capturing.
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                for (handle, name) in captures {
                    let shot = cx.update(|cx| {
                        handle.update(cx, |_, window, cx| {
                            window.render_frame(cx);
                            window.render_to_image()
                        })
                    });
                    match shot {
                        Ok(Ok(img)) => {
                            let path = format!("/tmp/lt-ui-{name}.png");
                            match img.save(&path) {
                                Ok(()) => println!("saved {path}"),
                                Err(e) => eprintln!("save {name} failed: {e}"),
                            }
                        }
                        other => eprintln!("capture {name} failed: {other:?}"),
                    }
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
}
