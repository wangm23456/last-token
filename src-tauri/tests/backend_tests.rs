use serde_json::json;
use last_token_lib::domain::{RiskState, HistoryPoint, ProviderConfig};
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
    assert!(settings.account_order.is_empty());

    // Save settings
    storage.update_settings(15).unwrap();
    let settings_new = storage.get_settings().unwrap();
    assert_eq!(settings_new.refresh_interval_minutes, 15);
    assert!(settings_new.account_order.is_empty());

    // Save account order independently
    storage
        .update_account_order(&vec!["b".into(), "a".into()])
        .unwrap();
    let settings_ordered = storage.get_settings().unwrap();
    assert_eq!(settings_ordered.refresh_interval_minutes, 15);
    assert_eq!(
        settings_ordered.account_order,
        vec!["b".to_string(), "a".to_string()]
    );
    storage.update_account_order(&[]).unwrap();
    assert!(storage.get_settings().unwrap().account_order.is_empty());

    // Save account & secrets
    let account_id = "test-account";
    let config = ProviderConfig::Kimi;
    let config_json = serde_json::to_string(&config).unwrap();
    storage.save_account(account_id, "kimi", "Kimi Account", true, "env", &config_json, None).unwrap();
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

    storage.save_account("acc", "kimi", "Kimi", true, "env", "{}", None).unwrap();

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

    storage.save_account("acc", "copilot", "Copilot", true, "env", "{}", None).unwrap();
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

#[test]
fn test_apply_account_order_manual_and_risk_fallback() {
    use last_token_lib::domain::{
        apply_account_order, sort_accounts_by_risk, AccountDashboard, CredentialSource,
        CredentialStatus, ProviderConfig, ProviderKind, PublicAccount, QuotaTier, RiskState,
        TierDashboard, TierForecast,
    };

    let make_acc = |id: &str, state: RiskState, exhaustion_at: Option<i64>| AccountDashboard {
        account: PublicAccount {
            id: id.into(),
            provider: ProviderKind::Claude,
            display_name: id.into(),
            enabled: true,
            credential_source: CredentialSource::Env,
            has_credential: true,
            config: ProviderConfig::Claude,
            alert_rules: vec![],
        },
        credential_status: CredentialStatus::Valid,
        stale: false,
        error: None,
        tiers: vec![TierDashboard {
            quota: QuotaTier {
                id: "five_hour".into(),
                label: "5h".into(),
                utilization: 50.0,
                resets_at: None,
                used: None,
                limit: None,
                unit: None,
                unlimited: false,
            },
            forecast: TierForecast {
                state,
                rate_per_hour: 1.0,
                projected_utilization_at_reset: 50.0,
                exhaustion_at,
                sample_count: 2,
                observation_minutes: 30,
            },
        }],
    };

    let safe = make_acc("safe", RiskState::Safe, None);
    let at_risk = make_acc("risk", RiskState::AtRisk, Some(1_000));
    let exhausted = make_acc("ex", RiskState::Exhausted, Some(500));
    let accounts = vec![safe.clone(), at_risk.clone(), exhausted.clone()];

    let by_risk = sort_accounts_by_risk(&accounts);
    assert_eq!(
        by_risk.iter().map(|a| a.account.id.as_str()).collect::<Vec<_>>(),
        vec!["ex", "risk", "safe"]
    );

    let manual = apply_account_order(&accounts, &vec!["safe".into(), "ex".into()]);
    assert_eq!(
        manual.iter().map(|a| a.account.id.as_str()).collect::<Vec<_>>(),
        vec!["safe", "ex", "risk"]
    );

    let empty = apply_account_order(&accounts, &[]);
    assert_eq!(
        empty.iter().map(|a| a.account.id.as_str()).collect::<Vec<_>>(),
        vec!["ex", "risk", "safe"]
    );
}

// ── Tray Pure Tests ──────────────────────────────────────────────

#[test]
fn test_tray_should_toggle_on_left_or_right_release() {
    use last_token_lib::tray::should_toggle_panel;
    use tauri::tray::{MouseButton, MouseButtonState};

    assert!(!should_toggle_panel(MouseButton::Left, MouseButtonState::Down));
    assert!(should_toggle_panel(MouseButton::Left, MouseButtonState::Up));
    assert!(should_toggle_panel(MouseButton::Right, MouseButtonState::Up));
    assert!(!should_toggle_panel(MouseButton::Right, MouseButtonState::Down));
}

#[test]
fn test_clamp_panel_height() {
    use last_token_lib::tray::{
        clamp_panel_height, TRAY_PANEL_MAX_HEIGHT, TRAY_PANEL_MIN_HEIGHT,
    };

    assert_eq!(clamp_panel_height(50.0), TRAY_PANEL_MIN_HEIGHT);
    assert_eq!(clamp_panel_height(900.0), TRAY_PANEL_MAX_HEIGHT);
    assert_eq!(clamp_panel_height(240.0), 240.0);
    assert_eq!(clamp_panel_height(f64::NAN), TRAY_PANEL_MIN_HEIGHT);
}

#[test]
fn test_tray_panel_position_top_menu_bar() {
    use last_token_lib::tray::{
        compute_panel_position, TRAY_PANEL_MAX_HEIGHT, TRAY_PANEL_WIDTH,
    };

    // 1000x800 work area, icon at top center (e.g. macOS menu bar)
    let (x, y) = compute_panel_position(
        (450.0, 10.0, 44.0, 22.0),   // icon rect
        (TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT), // panel size
        (0.0, 25.0, 1000.0, 775.0),  // work area (y=25 for menu bar)
    );
    assert!(y >= 10.0 + 22.0); // below the icon
    assert!(x >= 0.0 && x + TRAY_PANEL_WIDTH <= 1000.0);
    assert!(y + TRAY_PANEL_MAX_HEIGHT <= 25.0 + 775.0);
}

#[test]
fn test_tray_panel_position_bottom_taskbar() {
    use last_token_lib::tray::{
        compute_panel_position, TRAY_PANEL_MAX_HEIGHT, TRAY_PANEL_WIDTH,
    };

    // icon at bottom taskbar
    let (x, y) = compute_panel_position(
        (900.0, 740.0, 44.0, 22.0),
        (TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT),
        (0.0, 0.0, 1000.0, 800.0),
    );
    assert!(y + TRAY_PANEL_MAX_HEIGHT <= 740.0); // above the icon
    assert!(x + TRAY_PANEL_WIDTH <= 1000.0); // clamped to right edge
    assert!(x >= 0.0);
}

#[test]
fn test_tray_panel_position_negative_monitor_coords() {
    use last_token_lib::tray::{
        compute_panel_position, TRAY_PANEL_MAX_HEIGHT, TRAY_PANEL_WIDTH,
    };

    let (x, y) = compute_panel_position(
        (-460.0, 10.0, 44.0, 22.0), // icon on left external monitor
        (TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT),
        (-500.0, 0.0, 500.0, 800.0),
    );
    assert!(x >= -500.0 && x + TRAY_PANEL_WIDTH <= 0.0);
    assert!(y >= 0.0 && y + TRAY_PANEL_MAX_HEIGHT <= 800.0);
}

#[test]
fn test_tray_panel_position_right_edge_clamp() {
    use last_token_lib::tray::{
        compute_panel_position, TRAY_PANEL_MAX_HEIGHT, TRAY_PANEL_WIDTH,
    };

    let (x, _y) = compute_panel_position(
        (1200.0, 10.0, 44.0, 22.0), // near right edge
        (TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT),
        (0.0, 0.0, 1280.0, 800.0),
    );
    assert!(x + TRAY_PANEL_WIDTH <= 1280.0);
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
            None,
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



// ── Alert evaluation & persistence ───────────────────────────────

#[test]
fn test_is_quota_cycle_break_semantics() {
    use last_token_lib::forecast::is_quota_cycle_break;

    // utilization drop > 2
    assert!(is_quota_cycle_break(50.0, Some(1000), 47.0, Some(1000), 5));
    // small drop is not a break
    assert!(!is_quota_cycle_break(50.0, Some(1000), 48.5, Some(1000), 5));
    // resets_at jump beyond 2 * polling interval
    let polling = 5_i64;
    let threshold = polling * 2 * 60 * 1000;
    assert!(is_quota_cycle_break(50.0, Some(10_000), 51.0, Some(10_000 + threshold + 1), polling));
    // None ↔ Some alone is NOT a break
    assert!(!is_quota_cycle_break(50.0, None, 51.0, Some(10_000), polling));
    assert!(!is_quota_cycle_break(50.0, Some(10_000), 51.0, None, polling));
}

#[test]
fn test_save_account_metadata_preserves_snapshots_and_none_rules() {
    use last_token_lib::domain::AlertRule;
    use last_token_lib::storage::Storage;

    let tmp = tempfile::tempdir().unwrap();
    let db = Storage::new(tmp.path().join("alerts.db")).unwrap();
    db.save_account("acc1", "kimi", "Kimi", true, "env", "{\"type\":\"kimi\"}", None)
        .unwrap();
    db.insert_snapshot("acc1", "five_hour", 10.0, Some(999), 1000, None, None, None, false)
        .unwrap();

    // metadata-only update must keep snapshots
    db.save_account("acc1", "kimi", "Kimi Renamed", true, "env", "{\"type\":\"kimi\"}", None)
        .unwrap();
    let snaps = db.get_snapshots("acc1", "five_hour", 10).unwrap();
    assert_eq!(snaps.len(), 1);

    let rules = vec![AlertRule {
        tier_id: "five_hour".into(),
        enabled: true,
        threshold_percent: 80,
    }];
    db.save_account(
        "acc1",
        "kimi",
        "Kimi Renamed",
        true,
        "env",
        "{\"type\":\"kimi\"}",
        Some(&rules),
    )
    .unwrap();
    assert_eq!(db.list_alert_rules("acc1").unwrap().len(), 1);

    // None preserves rules
    db.save_account(
        "acc1",
        "kimi",
        "Kimi Again",
        false,
        "env",
        "{\"type\":\"kimi\"}",
        None,
    )
    .unwrap();
    assert_eq!(db.list_alert_rules("acc1").unwrap().len(), 1);

    // Some([]) clears rules
    db.save_account(
        "acc1",
        "kimi",
        "Kimi Again",
        false,
        "env",
        "{\"type\":\"kimi\"}",
        Some(&[]),
    )
    .unwrap();
    assert!(db.list_alert_rules("acc1").unwrap().is_empty());
}

#[test]
fn test_evaluate_alert_dedupe_and_exhaustion_priority() {
    use last_token_lib::alerts::{evaluate_alert, AlertEventKind, AlertState};
    use last_token_lib::domain::{AlertRule, QuotaTier, RiskState, TierDashboard, TierForecast};

    fn tier(util: f64, state: RiskState) -> TierDashboard {
        TierDashboard {
            quota: QuotaTier {
                id: "five_hour".into(),
                label: "5-Hour Session".into(),
                utilization: util,
                resets_at: Some(10_000_000),
                used: None,
                limit: None,
                unit: None,
                unlimited: false,
            },
            forecast: TierForecast {
                state,
                rate_per_hour: 1.5,
                projected_utilization_at_reset: util,
                exhaustion_at: None,
                sample_count: 5,
                observation_minutes: 30,
            },
        }
    }

    let rule = AlertRule {
        tier_id: "five_hour".into(),
        enabled: true,
        threshold_percent: 80,
    };

    // 79 -> 80 fires once
    let d1 = evaluate_alert(&rule, None, &tier(80.0, RiskState::Safe), 5);
    assert_eq!(d1.event, Some(AlertEventKind::ThresholdReached));

    let prev = AlertState {
        account_id: "a".into(),
        tier_id: "five_hour".into(),
        last_resets_at: Some(10_000_000),
        last_utilization: 80.0,
        threshold_notified: true,
        exhausted_notified: false,
    };
    let d2 = evaluate_alert(&rule, Some(&prev), &tier(85.0, RiskState::Safe), 5);
    assert_eq!(d2.event, None);

    // 80 -> 99.9 only exhausts
    let d3 = evaluate_alert(&rule, Some(&prev), &tier(99.9, RiskState::Exhausted), 5);
    assert_eq!(d3.event, Some(AlertEventKind::Exhausted));

    // direct jump to 99.9 with no previous -> Exhausted only
    let d4 = evaluate_alert(&rule, None, &tier(99.95, RiskState::Exhausted), 5);
    assert_eq!(d4.event, Some(AlertEventKind::Exhausted));

    // re-arm after utilization drop > 2
    let armed_prev = AlertState {
        account_id: "a".into(),
        tier_id: "five_hour".into(),
        last_resets_at: Some(10_000_000),
        last_utilization: 90.0,
        threshold_notified: true,
        exhausted_notified: true,
    };
    let d5 = evaluate_alert(&rule, Some(&armed_prev), &tier(80.0, RiskState::Safe), 5);
    assert_eq!(d5.event, Some(AlertEventKind::ThresholdReached));
    assert!(!d5.next_state.threshold_notified);
    assert!(!d5.next_state.exhausted_notified);
}

struct FakeSender {
    calls: std::sync::Mutex<Vec<(String, String)>>,
    fail: bool,
}

impl last_token_lib::alerts::NotificationSender for FakeSender {
    fn send(&self, title: &str, body: &str) -> Result<(), String> {
        if self.fail {
            return Err("boom".into());
        }
        self.calls.lock().unwrap().push((title.to_string(), body.to_string()));
        Ok(())
    }
}

#[test]
fn test_process_alerts_skips_stale_error_unlimited_and_isolates_accounts() {
    use last_token_lib::alerts::process_alerts;
    use last_token_lib::domain::{
        AccountDashboard, AlertRule, CredentialSource, CredentialStatus, DashboardSnapshot,
        ProviderConfig, ProviderKind, PublicAccount, QuotaTier, RiskState, TierDashboard,
        TierForecast,
    };
    use last_token_lib::storage::Storage;

    let tmp = tempfile::tempdir().unwrap();
    let db = Storage::new(tmp.path().join("process.db")).unwrap();

    db.save_account("good", "kimi", "Good", true, "env", "{\"type\":\"kimi\"}", None)
        .unwrap();
    db.save_account("bad", "kimi", "Bad", true, "env", "{\"type\":\"kimi\"}", None)
        .unwrap();
    db.save_account(
        "good",
        "kimi",
        "Good",
        true,
        "env",
        "{\"type\":\"kimi\"}",
        Some(&[AlertRule {
            tier_id: "five_hour".into(),
            enabled: true,
            threshold_percent: 50,
        }]),
    )
    .unwrap();
    db.save_account(
        "bad",
        "kimi",
        "Bad",
        true,
        "env",
        "{\"type\":\"kimi\"}",
        Some(&[AlertRule {
            tier_id: "five_hour".into(),
            enabled: true,
            threshold_percent: 50,
        }]),
    )
    .unwrap();

    let mk_tier = |id: &str, util: f64, unlimited: bool| TierDashboard {
        quota: QuotaTier {
            id: id.into(),
            label: id.into(),
            utilization: util,
            resets_at: Some(99_999),
            used: None,
            limit: None,
            unit: None,
            unlimited,
        },
        forecast: TierForecast {
            state: RiskState::Safe,
            rate_per_hour: 1.0,
            projected_utilization_at_reset: util,
            exhaustion_at: None,
            sample_count: 3,
            observation_minutes: 20,
        },
    };

    let mk_acc = |id: &str, name: &str, stale: bool, error: Option<&str>, tiers: Vec<TierDashboard>| {
        AccountDashboard {
            account: PublicAccount {
                id: id.into(),
                provider: ProviderKind::Kimi,
                display_name: name.into(),
                enabled: true,
                credential_source: CredentialSource::Env,
                has_credential: true,
                config: ProviderConfig::Kimi,
                alert_rules: vec![],
            },
            credential_status: CredentialStatus::Valid,
            stale,
            error: error.map(|s| s.to_string()),
            tiers,
        }
    };

    let snapshot = DashboardSnapshot {
        accounts: vec![
            mk_acc("good", "Good", false, None, vec![mk_tier("five_hour", 60.0, false)]),
            mk_acc("bad", "Bad", false, Some("provider failed"), vec![mk_tier("five_hour", 90.0, false)]),
        ],
        leading_risk: RiskState::Safe,
        refreshed_at: 1,
        next_refresh_at: 2,
        refresh_in_progress: false,
    };

    let sender = FakeSender {
        calls: std::sync::Mutex::new(Vec::new()),
        fail: false,
    };
    process_alerts(&db, &sender, &snapshot, 5).unwrap();
    let calls = sender.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("Good"));
    assert!(!calls[0].0.contains("Bad"));

    // second pass same cycle -> no resend
    process_alerts(&db, &sender, &snapshot, 5).unwrap();
    assert_eq!(sender.calls.lock().unwrap().len(), 1);

    // state persisted
    let st = db.get_alert_state("good", "five_hour").unwrap().unwrap();
    assert!(st.threshold_notified);

    // rule change clears state
    db.save_account(
        "good",
        "kimi",
        "Good",
        true,
        "env",
        "{\"type\":\"kimi\"}",
        Some(&[AlertRule {
            tier_id: "five_hour".into(),
            enabled: true,
            threshold_percent: 70,
        }]),
    )
    .unwrap();
    assert!(db.get_alert_state("good", "five_hour").unwrap().is_none());
}
