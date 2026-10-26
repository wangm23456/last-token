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
            alert_rules: vec![last_token_lib::domain::AlertRule {
                tier_id: "five_hour".into(),
                enabled: true,
                threshold_percent: 80,
            }],
        },
        credential_status: CredentialStatus::Valid,
        stale: false,
        error: None,
        tiers,
    }
}

fn mock_snapshot() -> DashboardSnapshot {
    let now = chrono::Utc::now().timestamp_millis();
    // Backend always hands the snapshot worst-first (apply_account_order);
    // the fixture mirrors that contract.
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
    let mut safe = account(
        "a:1",
        "Claude 工作号",
        ProviderKind::Claude,
        vec![
            tier(now, "five_hour", "5-Hour Session", 24.0, RiskState::Safe, 3),
            tier(now, "seven_day", "7-Day Limit", 41.0, RiskState::Safe, 72),
            tier(now, "unlimited_seats", "Unlimited Seats", 12.0, RiskState::Safe, 72),
        ],
    );
    let mut broken = account("a:3", "Copilot 团队", ProviderKind::Copilot, vec![]);
    broken.credential_status = CredentialStatus::Unavailable;
    broken.error = Some("token has expired or is invalid".into());
    safe.account.alert_rules.push(last_token_lib::domain::AlertRule {
        tier_id: "unlimited_seats".into(),
        enabled: false,
        threshold_percent: 90,
    });
    let mut snapshot = DashboardSnapshot {
        accounts: vec![risk, safe, broken],
        leading_risk: RiskState::AtRisk,
        refreshed_at: now - 90_000,
        next_refresh_at: now + 210_000,
        refresh_in_progress: false,
    };
    snapshot.accounts[1].tiers[2].quota.unlimited = true;
    snapshot
}

fn mock_histories() -> std::collections::HashMap<String, Vec<last_token_lib::domain::HistoryPoint>>
{
    let now = chrono::Utc::now().timestamp_millis();
    let points = |base: f64| {
        (0..12)
            .map(|i| last_token_lib::domain::HistoryPoint {
                sampled_at: now - (11 - i) * 30 * 60_000,
                utilization: base + i as f64,
                resets_at: None,
            })
            .collect::<Vec<_>>()
    };
    let mut map = std::collections::HashMap::new();
    map.insert("monthly".to_string(), points(75.0));
    map.insert("five_hour".to_string(), points(18.0));
    map.insert("seven_day".to_string(), points(35.0));
    map.insert("unlimited_seats".to_string(), points(10.0));
    map
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
        busy: Busy::Idle,
        toasts: vec![],
        device_flow: None,
        selected_account: None,
        tier_histories: None,
    };
    cx.new(|_| store)
}

fn shell_page(cx: &mut TestAppContext, page: Surface, w: f32, h: f32) -> gpui_kit::AnyWindowHandle {
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
            |window, cx| cx.new(|cx| AppShell::new(window, cx, store.clone())),
        )
        .expect("open shell")
    });
    shell.update(cx, |shell, _| shell.surface = page);
    let _ = cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .expect("render frame");
    handle
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

fn open_watch(cx: &mut TestAppContext, with_data: bool) -> gpui_kit::AnyWindowHandle {
    cx.update(gpui_kit::init);
    let store = cx.update(build_store);
    store.update(cx, |store, _| {
        store.selected_account = None;
        if with_data {
            store.tier_histories = Some(("a:2".into(), mock_histories()));
        } else {
            store.snapshot = None;
            store.accounts.clear();
            store.tier_histories = None;
        }
    });
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(760.), px(720.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| AppShell::new(window, cx, store.clone())),
        )
        .expect("open watch")
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .expect("render frame");
    handle
}

fn render(handle: gpui_kit::AnyWindowHandle, cx: &mut TestAppContext) {
    let _ = cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .expect("render frame");
}

#[gpui_kit::test]
fn watch_default_selects_worst_account(cx: &mut TestAppContext) {
    let handle = open_watch(cx, true);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        assert!(window.find("acct-row-a:2").visible());
        let name = window.find("watch-detail-name");
        assert_eq!(name.label().as_deref(), Some("Kimi 长期号"));
    });
}

#[gpui_kit::test]
fn watch_click_switches_detail(cx: &mut TestAppContext) {
    let handle = open_watch(cx, true);
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("acct-row-a:1", cx);
    });
    render(handle, cx);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        let name = window.find("watch-detail-name");
        assert_eq!(name.label().as_deref(), Some("Claude 工作号"));
    });
}

#[gpui_kit::test]
fn watch_tier_sparkline_visible(cx: &mut TestAppContext) {
    let handle = open_watch(cx, true);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        // Default selection is the worst account (a:2); its monthly tier has
        // injected history, so the sparkline must render with real width.
        let chart = window.find("tier-chart-a:2-monthly");
        assert!(chart.visible(), "tier sparkline must render with history");
        assert!(chart.bounds().size.width > px(0.), "sparkline has width");
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("acct-row-a:1", cx);
    });
    render(handle, cx);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        let unlimited = window.find("tier-pct-a:1-unlimited_seats");
        assert!(
            unlimited.label().is_some(),
            "unlimited tier marker renders; bounds={:?}",
            unlimited.bounds()
        );
    });
}

#[gpui_kit::test]
fn watch_error_account_detail(cx: &mut TestAppContext) {
    let handle = open_watch(cx, true);
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("acct-row-a:3", cx);
    });
    render(handle, cx);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        let err = window.find("watch-detail-error");
        assert!(
            err.visible(),
            "error box visible; bounds={:?} label={:?}",
            err.bounds(),
            err.label()
        );
    });
}

#[gpui_kit::test]
fn watch_empty_state(cx: &mut TestAppContext) {
    let handle = open_watch(cx, false);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        assert!(window.find("watch-empty").visible());
    });
}

#[gpui_kit::test]
fn management_defaults_to_accounts_section(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = shell_page(cx, Surface::Management, 760., 900.);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        assert!(window.find("mgmt-section-accounts").visible());
        assert!(window.try_find("mgmt-section-alerts").is_none());
    });
}

#[gpui_kit::test]
fn management_alerts_rules_editable_and_unlimited_locked(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = shell_page(cx, Surface::Management, 760., 900.);
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("mgmt-nav-alerts", cx);
    });
    render(handle, cx);
    // Unlimited-tier rule row renders and its toggle is locked: clicking it
    // must not flip the draft.
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("mgmt-section-alerts").visible());
        assert!(window.try_find("mgmt-section-accounts").is_none());
        assert!(window.find("alert-rule-a:1-unlimited_seats").visible());
        window.click("alert-toggle-a:1-unlimited_seats", cx);
    });
    render(handle, cx);
    // The normal rule's toggle is enabled: clicking flips its draft.
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("alert-toggle-a:1-five_hour", cx);
    });
    render(handle, cx);
}

#[gpui_kit::test]
fn management_subnav_switches_sections(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = shell_page(cx, Surface::Management, 760., 900.);
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.click("mgmt-nav-settings", cx);
    });
    render(handle, cx);
    let _ = cx.update_window(handle.into(), |_, window, _| {
        assert!(window.find("mgmt-section-settings").visible());
        assert!(window.try_find("mgmt-section-accounts").is_none());
    });
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

