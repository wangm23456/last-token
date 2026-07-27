use std::collections::HashMap;
use serde_json::json;
use last_token_lib::domain::{RiskState, HistoryPoint, ProviderKind, ProviderConfig};
use last_token_lib::forecast::compute_forecast;
use last_token_lib::providers::{
    parse_claude_quota_body, parse_codex_quota_body, parse_gemini_quota_body,
    parse_copilot_quota_body, parse_kimi_quota_body, parse_zhipu_quota_body,
    parse_minimax_quota_body, parse_zenmux_quota_body, parse_afp_tiers,
    parse_coding_plan_tiers, volcengine_sign,
};
use last_token_lib::storage::{Storage, InMemorySecretStore, SecretStore};

// ── Forecast Tests ───────────────────────────────────────────────

#[test]
fn test_forecast_unlimited() {
    let history = vec![];
    let forecast = compute_forecast(true, 50.0, Some(1000), &history, 500, 5);
    assert_eq!(forecast.state, RiskState::Safe);
    assert_eq!(forecast.rate_per_hour, 0.0);
    assert!(forecast.exhaustion_at.is_none());
}

#[test]
fn test_forecast_exhausted() {
    let history = vec![];
    let forecast = compute_forecast(false, 99.95, Some(1000), &history, 500, 5);
    assert_eq!(forecast.state, RiskState::Exhausted);
    assert_eq!(forecast.exhaustion_at, Some(500));
}

#[test]
fn test_forecast_unknown_reset() {
    let history = vec![];
    let forecast = compute_forecast(false, 50.0, None, &history, 500, 5);
    assert_eq!(forecast.state, RiskState::UnknownReset);
    assert!(forecast.exhaustion_at.is_none());
    
    // Past reset
    let forecast_past = compute_forecast(false, 50.0, Some(400), &history, 500, 5);
    assert_eq!(forecast_past.state, RiskState::UnknownReset);
}

#[test]
fn test_forecast_learning() {
    let history = vec![
        HistoryPoint { sampled_at: 100, utilization: 10.0, resets_at: Some(1000) },
        HistoryPoint { sampled_at: 200, utilization: 20.0, resets_at: Some(1000) },
    ];
    // Insufficient samples (< 3)
    let forecast = compute_forecast(false, 20.0, Some(1000), &history, 500, 5);
    assert_eq!(forecast.state, RiskState::Learning);
    
    // Insufficient span (< 15 mins)
    let history_short = vec![
        HistoryPoint { sampled_at: 100, utilization: 10.0, resets_at: Some(1000) },
        HistoryPoint { sampled_at: 110, utilization: 20.0, resets_at: Some(1000) },
        HistoryPoint { sampled_at: 120, utilization: 30.0, resets_at: Some(1000) },
    ];
    let forecast_short = compute_forecast(false, 30.0, Some(1000), &history_short, 500, 5);
    assert_eq!(forecast_short.state, RiskState::Learning);
}

#[test]
fn test_forecast_safe_and_at_risk() {
    // 3 samples spanning 30 minutes
    // rate = 20% / 0.5 hour = 40% per hour
    let history = vec![
        HistoryPoint { sampled_at: 10_000_000, utilization: 40.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 10_900_000, utilization: 50.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 11_800_000, utilization: 60.0, resets_at: Some(15_000_000) },
    ];
    
    // At risk: rate is 40%/h. Under 40%/h, remaining 40% will be exhausted in 1 hour.
    // Exhaustion will be at 11_800_000 + 3,600,000 = 15,400,000.
    // Reset is at 15_000_000, which is BEFORE exhaustion (Wait: if exhaustion is after reset, it is safe!).
    // Let's verify: resets_at = 15_000_000. exhaustion_at = 15_400_000.
    // Since resets_at < exhaustion_at, the quota resets before we run out, so it should be SAFE!
    let forecast_safe = compute_forecast(false, 60.0, Some(15_000_000), &history, 11_800_000, 5);
    assert_eq!(forecast_safe.state, RiskState::Safe);
    
    // Let's make it At Risk: reset is at 16_000_000.
    // Wait, if reset is at 16_000_000, and exhaustion is at 15_400_000.
    // Since exhaustion_at <= resets_at, we run out BEFORE reset, so it is AT RISK!
    let forecast_risk = compute_forecast(false, 60.0, Some(16_000_000), &history, 11_800_000, 5);
    assert_eq!(forecast_risk.state, RiskState::AtRisk);
    assert!(forecast_risk.rate_per_hour > 0.0);
}

#[test]
fn test_forecast_flat_safe() {
    let history = vec![
        HistoryPoint { sampled_at: 10_000_000, utilization: 40.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 10_900_000, utilization: 40.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 11_800_000, utilization: 40.0, resets_at: Some(15_000_000) },
    ];
    let forecast = compute_forecast(false, 40.0, Some(15_000_000), &history, 11_800_000, 5);
    assert_eq!(forecast.state, RiskState::Safe);
    assert_eq!(forecast.rate_per_hour, 0.0);
    assert!(forecast.exhaustion_at.is_none());
}

#[test]
fn test_forecast_segmentation_break() {
    // There is a drop in utilization > 2% between sample 2 and 3.
    // Or resets_at changes.
    // This should start a new segment, making the remaining samples too few.
    let history = vec![
        HistoryPoint { sampled_at: 10_000_000, utilization: 40.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 10_900_000, utilization: 50.0, resets_at: Some(15_000_000) },
        HistoryPoint { sampled_at: 11_800_000, utilization: 41.0, resets_at: Some(15_000_000) }, // drop of 9%
        HistoryPoint { sampled_at: 12_700_000, utilization: 45.0, resets_at: Some(15_000_000) },
    ];
    let forecast = compute_forecast(false, 45.0, Some(15_000_000), &history, 12_700_000, 5);
    // Only last 2 samples in segment, so it should return Learning
    assert_eq!(forecast.state, RiskState::Learning);
}

// ── Provider Parser Tests ────────────────────────────────────────

#[test]
fn test_parse_claude() {
    let body = json!({
        "five_hour": {
            "utilization": 23.4,
            "resetsAt": "2026-07-23T15:30:00Z"
        },
        "seven_day": {
            "utilization": 80.0,
            "resets_at": 1784793600000i64
        }
    });
    let tiers = parse_claude_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 23.4);
    assert_eq!(tiers[1].id, "seven_day");
    assert_eq!(tiers[1].utilization, 80.0);
}

#[test]
fn test_parse_codex() {
    let body = json!({
        "rate_limit": {
            "primary_window": {
                "used_percent": 12.5,
                "limit_window_seconds": 18000,
                "reset_at": 1784793600
            },
            "secondary_window": {
                "used_percent": 95.0,
                "limit_window_seconds": 2592000,
                "reset_at": "2026-07-23T16:00:00Z"
            }
        }
    });
    let tiers = parse_codex_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 12.5);
    assert_eq!(tiers[1].id, "30_day");
    assert_eq!(tiers[1].utilization, 95.0);
}

#[test]
fn test_parse_gemini() {
    let body = json!({
        "buckets": [
            {
                "modelId": "models/gemini-1.5-pro",
                "remainingFraction": 0.4,
                "resetTime": 1784793600000i64
            },
            {
                "modelId": "models/gemini-1.5-flash",
                "remainingFraction": 0.9,
                "resetTime": "2026-07-23T15:00:00Z"
            }
        ]
    });
    let tiers = parse_gemini_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "gemini_pro");
    assert_eq!(tiers[0].utilization, 60.0);
    assert_eq!(tiers[1].id, "gemini_flash");
    assert!((tiers[1].utilization - 10.0).abs() < 1e-9);
}

#[test]
fn test_parse_copilot() {
    let body = json!({
        "copilot_plan": "Copilot Business",
        "quota_reset_date": "2026-07-23T18:00:00Z",
        "quota_snapshots": {
            "premium_interactions": {
                "entitlement": 500,
                "remaining": 400,
                "unlimited": false
            },
            "chat": {
                "entitlement": 100,
                "remaining": 100,
                "unlimited": true
            },
            "completions": {
                "entitlement": 100,
                "remaining": 100,
                "unlimited": true
            }
        }
    });
    let tiers = parse_copilot_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 1);
    assert_eq!(tiers[0].id, "monthly");
    assert_eq!(tiers[0].utilization, 20.0);
    assert_eq!(tiers[0].used, Some(100.0));
    assert_eq!(tiers[0].limit, Some(500.0));
    assert_eq!(tiers[0].unlimited, false);
}

#[test]
fn test_parse_kimi() {
    let body = json!({
        "limits": [
            {
                "detail": {
                    "limit": 100,
                    "remaining": 80,
                    "resetTime": "2026-07-23T15:30:00Z"
                }
            }
        ],
        "usage": {
            "limit": 1000,
            "remaining": 900,
            "resetTime": 1784793600000i64
        }
    });
    let tiers = parse_kimi_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 20.0);
    assert_eq!(tiers[1].id, "weekly_limit");
    assert_eq!(tiers[1].utilization, 10.0);
}

#[test]
fn test_parse_zhipu() {
    let body = json!({
        "success": true,
        "data": {
            "level": "Pro",
            "limits": [
                {
                    "type": "TOKENS_LIMIT",
                    "unit": 3,
                    "percentage": 45.0,
                    "nextResetTime": 1784793600000i64
                },
                {
                    "type": "TOKENS_LIMIT",
                    "unit": 6,
                    "percentage": 10.0,
                    "nextResetTime": 1784793600000i64
                }
            ]
        }
    });
    let tiers = parse_zhipu_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 45.0);
    assert_eq!(tiers[1].id, "weekly_limit");
    assert_eq!(tiers[1].utilization, 10.0);
}

#[test]
fn test_parse_minimax() {
    let body = json!({
        "base_resp": {
            "status_code": 0
        },
        "model_remains": [
            {
                "model_name": "general",
                "current_interval_remaining_percent": 85.0,
                "end_time": 1784793600000i64,
                "current_weekly_status": 1,
                "current_weekly_remaining_percent": 95.0,
                "weekly_end_time": 1784793600000i64
            }
        ]
    });
    let tiers = parse_minimax_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 15.0);
    assert_eq!(tiers[1].id, "weekly_limit");
    assert_eq!(tiers[1].utilization, 5.0);
}

#[test]
fn test_parse_zenmux() {
    let body = json!({
        "success": true,
        "data": {
            "plan": {
                "tier": "Developer"
            },
            "quota_5_hour": {
                "usage_percentage": 0.15,
                "resets_at": "2026-07-23T15:30:00Z",
                "used_value_usd": 1.5,
                "max_value_usd": 10.0
            },
            "quota_7_day": {
                "usage_percentage": 0.50,
                "resets_at": "2026-07-23T15:30:00Z",
                "used_value_usd": 5.0,
                "max_value_usd": 10.0
            }
        }
    });
    let tiers = parse_zenmux_quota_body(&body).unwrap();
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 15.0);
    assert_eq!(tiers[0].used, Some(1.5));
    assert_eq!(tiers[0].limit, Some(10.0));
    assert_eq!(tiers[1].id, "weekly_limit");
    assert_eq!(tiers[1].utilization, 50.0);
}

#[test]
fn test_parse_volcengine() {
    let body_afp = json!({
        "Result": {
            "PlanType": "Premium",
            "AFPFiveHour": {
                "Quota": 1000.0,
                "Used": 300.0,
                "ResetTime": 1784793600000i64
            },
            "AFPWeekly": {
                "Quota": 10000.0,
                "Used": 1000.0,
                "ResetTime": 1784793600000i64
            }
        }
    });
    let tiers = parse_afp_tiers(&body_afp.get("Result").unwrap());
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0].id, "five_hour");
    assert_eq!(tiers[0].utilization, 30.0);
    assert_eq!(tiers[1].id, "weekly_limit");
    assert_eq!(tiers[1].utilization, 10.0);

    let body_coding = json!({
        "QuotaUsage": [
            {
                "Level": "session",
                "Percent": 15.0,
                "ResetTime": 1784793600000i64
            },
            {
                "Level": "weekly",
                "Percent": 5.0,
                "ResetTime": 1784793600000i64
            }
        ]
    });
    let tiers_coding = parse_coding_plan_tiers(&body_coding);
    assert_eq!(tiers_coding.len(), 2);
    assert_eq!(tiers_coding[0].id, "five_hour");
    assert_eq!(tiers_coding[0].utilization, 15.0);
    assert_eq!(tiers_coding[1].id, "weekly_limit");
    assert_eq!(tiers_coding[1].utilization, 5.0);
}

// ── Volcengine Signature Test ────────────────────────────────────

#[test]
fn test_volcengine_sign_golden() {
    let ak = "test_ak";
    let sk = "test_sk";
    let region = "cn-beijing";
    let query = "Action=GetAFPUsage&Region=cn-beijing&Version=2024-01-01";
    let body = b"";
    let now = chrono::DateTime::parse_from_rfc3339("2026-07-23T06:00:00Z").unwrap().with_timezone(&chrono::Utc);

    let (auth, x_date, x_content_sha256) = volcengine_sign(ak, sk, region, query, body, now);
    
    assert_eq!(x_date, "20260723T060000Z");
    assert_eq!(x_content_sha256, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"); // SHA256 of empty body
    assert!(auth.contains("HMAC-SHA256"));
    assert!(auth.contains("Credential=test_ak/20260723/cn-beijing/ark/request"));
}

// ── Storage Tests ────────────────────────────────────────────────

#[test]
fn test_storage_and_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test-token.sqlite3");
    let storage = Storage::new(db_path).unwrap();
    let secret_store = InMemorySecretStore::new();

    // Verify settings default
    let settings = storage.get_settings().unwrap();
    assert_eq!(settings.refresh_interval_minutes, 5);

    // Save settings
    storage.update_settings(15).unwrap();
    let settings_new = storage.get_settings().unwrap();
    assert_eq!(settings_new.refresh_interval_minutes, 15);

    // Save account & secrets
    let account_id = "test-account";
    let config = ProviderConfig::Kimi;
    let config_json = serde_json::to_string(&config).unwrap();
    storage.save_account(account_id, "kimi", "Kimi Account", true, "env", &config_json).unwrap();
    secret_store.set_secret(account_id, "api_key", "kimi_secret_123").unwrap();

    // Verify list
    let list = storage.list_accounts_raw().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, "test-account");
    assert_eq!(list[0].2, "Kimi Account");

    // Verify secret
    let sec = secret_store.get_secret(account_id, "api_key").unwrap();
    assert_eq!(sec, Some("kimi_secret_123".to_string()));

    // Insert snapshot
    storage.insert_snapshot(account_id, "five_hour", 35.5, Some(1784793600000), 100000, None, None, None, false).unwrap();
    let history = storage.get_snapshots(account_id, "five_hour", 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].utilization, 35.5);

    // Delete account
    storage.delete_account(account_id).unwrap();
    secret_store.delete_secret(account_id, "api_key").unwrap();

    let list_empty = storage.list_accounts_raw().unwrap();
    assert!(list_empty.is_empty());
    let sec_empty = secret_store.get_secret(account_id, "api_key").unwrap();
    assert!(sec_empty.is_none());
}

// ── Storage Regression Tests ─────────────────────────────────────

#[test]
fn test_storage_latest_batch_tier_ids_and_ordering() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().join("test.sqlite3")).unwrap();

    storage.save_account("acc", "kimi", "Kimi", true, "env", "{}").unwrap();

    let old_ts = 1000i64;
    let new_ts = 2000i64;

    // old batch with a tier that will not appear in the latest batch
    storage.insert_snapshot("acc", "five_hour", 10.0, None, old_ts, None, None, None, false).unwrap();
    storage.insert_snapshot("acc", "legacy_weekly", 20.0, None, old_ts, None, None, None, false).unwrap();

    // new batch
    storage.insert_snapshot("acc", "weekly_limit", 30.0, None, new_ts, None, None, None, false).unwrap();
    storage.insert_snapshot("acc", "five_hour", 15.0, None, new_ts, None, None, None, false).unwrap();
    storage.insert_snapshot("acc", "monthly", 5.0, None, new_ts, None, None, None, false).unwrap();

    let mut ids = storage.get_current_tier_ids("acc").unwrap();
    ids.sort_by_key(|id| last_token_lib::domain::tier_sort_key(id));

    assert_eq!(ids, vec!["five_hour", "weekly_limit", "monthly"]);
    assert!(!ids.contains(&"legacy_weekly".to_string()));
}

#[test]
fn test_storage_metadata_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().join("test.sqlite3")).unwrap();

    storage.save_account("acc", "copilot", "Copilot", true, "env", "{}").unwrap();
    storage
        .insert_snapshot(
            "acc",
            "monthly",
            20.0,
            Some(1784793600000),
            1000,
            Some(100.0),
            Some(500.0),
            Some("credits"),
            false,
        )
        .unwrap();

    let latest = storage.get_latest_snapshot("acc", "monthly").unwrap().unwrap();
    assert_eq!(latest.utilization, 20.0);
    assert_eq!(latest.used, Some(100.0));
    assert_eq!(latest.limit_value, Some(500.0));
    assert_eq!(latest.unit, Some("credits".to_string()));
    assert!(!latest.unlimited);

    storage
        .insert_snapshot(
            "acc",
            "monthly",
            0.0,
            Some(1784793600000),
            2000,
            None,
            None,
            None,
            true,
        )
        .unwrap();
    let latest = storage.get_latest_snapshot("acc", "monthly").unwrap().unwrap();
    assert_eq!(latest.utilization, 0.0);
    assert!(latest.unlimited);
    assert!(latest.used.is_none());
}

#[test]
fn test_forecast_zero_percent_finite_is_learning_not_unlimited() {
    // A 0% finite quota with not enough samples should be Learning, not Safe.
    let history = vec![HistoryPoint {
        sampled_at: 100,
        utilization: 0.0,
        resets_at: Some(1000),
    }];
    let forecast = compute_forecast(false, 0.0, Some(1000), &history, 500, 5);
    assert_eq!(forecast.state, RiskState::Learning);
    assert!(forecast.exhaustion_at.is_none());
}

#[test]
fn test_risk_severity_ordering_independent_of_enum_decl() {
    use last_token_lib::domain::risk_severity;
    assert!(risk_severity(RiskState::Exhausted) > risk_severity(RiskState::AtRisk));
    assert!(risk_severity(RiskState::AtRisk) > risk_severity(RiskState::UnknownReset));
    assert!(risk_severity(RiskState::UnknownReset) > risk_severity(RiskState::Error));
    assert!(risk_severity(RiskState::Error) > risk_severity(RiskState::Learning));
    assert!(risk_severity(RiskState::Learning) > risk_severity(RiskState::Safe));
}

#[test]
fn test_tier_sort_key_ordering() {
    use last_token_lib::domain::tier_sort_key;
    let mut ids = vec!["monthly", "five_hour", "weekly_limit", "gemini_pro", "30_day", "unknown_tier"];
    ids.sort_by_key(|id| tier_sort_key(id));
    assert_eq!(
        ids,
        vec!["five_hour", "gemini_pro", "weekly_limit", "30_day", "monthly", "unknown_tier"]
    );
}

#[test]
fn test_leading_tier_selects_worst_with_tie_breaks() {
    use last_token_lib::domain::{leading_tier, TierDashboard, TierForecast, QuotaTier};

    let make = |state: RiskState, utilization: f64, ex: Option<i64>, resets: Option<i64>| TierDashboard {
        quota: QuotaTier {
            id: "t".into(),
            label: "T".into(),
            utilization,
            resets_at: resets,
            used: None,
            limit: None,
            unit: None,
            unlimited: false,
        },
        forecast: TierForecast {
            state,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: 0.0,
            exhaustion_at: ex,
            sample_count: 0,
            observation_minutes: 0,
        },
    };

    let safe = make(RiskState::Safe, 10.0, None, None);
    let exhausted = make(RiskState::Exhausted, 99.0, Some(100), None);
    let at_risk_a = make(RiskState::AtRisk, 80.0, Some(200), None);
    let at_risk_b = make(RiskState::AtRisk, 90.0, Some(300), None);

    let tiers = vec![safe, exhausted.clone(), at_risk_a.clone(), at_risk_b.clone()];
    let worst = leading_tier(&tiers).unwrap();
    assert_eq!(worst.forecast.state, RiskState::Exhausted);

    let tiers = vec![at_risk_a, at_risk_b];
    let worst = leading_tier(&tiers).unwrap();
    assert_eq!(worst.quota.utilization, 90.0);
}

// ── Tray Pure Tests ──────────────────────────────────────────────

#[test]
fn test_tray_should_only_toggle_on_left_release() {
    use last_token_lib::tray::should_toggle_panel;
    use tauri::tray::{MouseButton, MouseButtonState};

    assert!(!should_toggle_panel(MouseButton::Left, MouseButtonState::Down));
    assert!(should_toggle_panel(MouseButton::Left, MouseButtonState::Up));
    assert!(!should_toggle_panel(MouseButton::Right, MouseButtonState::Up));
    assert!(!should_toggle_panel(MouseButton::Right, MouseButtonState::Down));
}

#[test]
fn test_tray_panel_position_top_menu_bar() {
    use last_token_lib::tray::compute_panel_position;

    // 1000x800 work area, icon at top center (e.g. macOS menu bar)
    let (x, y) = compute_panel_position(
        (450.0, 10.0, 44.0, 22.0),   // icon rect
        (380.0, 520.0),               // panel size
        (0.0, 25.0, 1000.0, 775.0),  // work area (y=25 for menu bar)
    );
    assert!(y >= 10.0 + 22.0); // below the icon
    assert!(x >= 0.0 && x + 380.0 <= 1000.0);
    assert!(y + 520.0 <= 25.0 + 775.0);
}

#[test]
fn test_tray_panel_position_bottom_taskbar() {
    use last_token_lib::tray::compute_panel_position;

    // icon at bottom taskbar
    let (x, y) = compute_panel_position(
        (900.0, 740.0, 44.0, 22.0),
        (380.0, 520.0),
        (0.0, 0.0, 1000.0, 800.0),
    );
    assert!(y + 520.0 <= 740.0); // above the icon
    assert!(x + 380.0 <= 1000.0); // clamped to right edge
    assert!(x >= 0.0);
}

#[test]
fn test_tray_panel_position_negative_monitor_coords() {
    use last_token_lib::tray::compute_panel_position;

    let (x, y) = compute_panel_position(
        (-460.0, 10.0, 44.0, 22.0), // icon on left external monitor
        (380.0, 520.0),
        (-500.0, 0.0, 500.0, 800.0),
    );
    assert!(x >= -500.0 && x + 380.0 <= 0.0);
    assert!(y >= 0.0 && y + 520.0 <= 800.0);
}

#[test]
fn test_tray_panel_position_right_edge_clamp() {
    use last_token_lib::tray::compute_panel_position;

    let (x, _y) = compute_panel_position(
        (1200.0, 10.0, 44.0, 22.0), // near right edge
        (380.0, 520.0),
        (0.0, 0.0, 1280.0, 800.0),
    );
    assert!(x + 380.0 <= 1280.0);
    assert!(x >= 0.0);
}

#[test]
fn test_keyring_source_migrated_to_env() {
    use last_token_lib::storage::Storage;

    let tmp_dir = tempfile::tempdir().unwrap();
    let db_path = tmp_dir.path().join("migrate.db");

    let storage = Storage::new(db_path.clone()).unwrap();
    storage
        .save_account(
            "kimi:legacy-keyring",
            "kimi",
            "Kimi (legacy)",
            true,
            "keyring",
            "{\"type\":\"kimi\"}",
        )
        .unwrap();
    drop(storage);

    let storage2 = Storage::new(db_path).unwrap();
    let rows = storage2.list_accounts_raw().unwrap();
    let (_, _, _, _, source_str, _) = rows
        .into_iter()
        .find(|(id, _, _, _, _, _)| id == "kimi:legacy-keyring")
        .expect("legacy account should still exist");
    assert_eq!(source_str, "env");
}

#[test]
fn test_credential_source_unknown_deserializes_fails() {
    assert!(serde_json::from_str::<last_token_lib::domain::CredentialSource>("\"keyring\"").is_err());
}

