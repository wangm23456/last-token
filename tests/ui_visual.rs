//! Visual smoke: opens each real view headless with mock data and renders a
//! frame, catching render panics. PNG capture lives in `cargo run --features
//! preview --bin preview` (needs the real platform renderer).

use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions};

use last_token_lib::domain::{
    AccountDashboard, CredentialSource, CredentialStatus, DashboardSnapshot, ProviderConfig,
    ProviderKind, PublicAccount, QuotaTier, RiskState, Settings, TierDashboard, TierForecast,
};
use last_token_lib::ui::shell::{AppShell, Surface};
use last_token_lib::ui::store::{AppStore, Busy};
use last_token_lib::ui::tray::StoreGlobal;
use last_token_lib::ui::tray_panel::TrayPanelView;

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

fn account(
    id: &str,
    name: &str,
    provider: ProviderKind,
    tiers: Vec<TierDashboard>,
) -> AccountDashboard {
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
        vec![tier(
            now,
            "monthly",
            "Monthly Limit",
            82.0,
            RiskState::AtRisk,
            240,
        )],
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
    // Unique per store: tests run in parallel in one process and would
    // otherwise share one SQLite file and deadlock on it.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "lt-ui-test-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let core = last_token_lib::service::Core::new_in(dir).expect("core");
    let snapshot = mock_snapshot();
    let store = AppStore {
        core,
        language: "zh-CN".into(),
        accounts: snapshot
            .accounts
            .iter()
            .map(|a| a.account.clone())
            .collect(),
        snapshot: Some(snapshot),
        settings: Settings {
            refresh_interval_minutes: 5,
            account_order: vec![],
        },
        probes: vec![],
        history: None,
        busy: Busy::Idle,
        toasts: vec![],
        device_flow: None,
    };
    cx.new(|_| store)
}

fn shell_page(cx: &mut TestAppContext, page: Surface, w: f32, h: f32) {
    let store = cx.update(build_store);
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(w), px(h)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.set_global(StoreGlobal(store.clone()));
                cx.new(|cx| AppShell::new(window, cx, store.clone()))
            },
        )
        .expect("open shell")
    });
    shell.update(cx, |shell, _| shell.surface = page);
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .expect("render frame");
}

#[gpui_kit::test]
fn watch_renders(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    // Watch is the default surface; no explicit set.
    shell_page(cx, Surface::Watch, 760., 720.);
}

#[gpui_kit::test]
fn management_renders(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    shell_page(cx, Surface::Management, 760., 900.);
}

#[test]
fn surface_labels_localized() {
    // The two-segment switch must resolve real copy in both locales, not
    // fall back to raw i18n keys.
    for lang in ["zh-CN", "en"] {
        for key in ["tabs.watch", "tabs.management"] {
            let label = last_token_lib::i18n::tr(lang, key);
            assert_ne!(label, key, "missing i18n key {key} for {lang}");
        }
    }
}

#[gpui_kit::test]
fn tray_panel_renders(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let store = cx.update(build_store);
    let (handle, _) = cx.update(|cx| {
        cx.set_global(StoreGlobal(store.clone()));
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(254.), px(520.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|cx| TrayPanelView::new(store.clone(), cx)),
        )
        .expect("open tray panel")
    });
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .expect("render frame");
}
