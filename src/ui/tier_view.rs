//! Tier presentation helpers: sorting, status semantics, formatters, and the
//! reusable quota-tier row. Ported from the React `QuotaTierList.tsx`.

use gpui_kit::component::progress::Progress;
use gpui_kit::component::theme::Theme;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use crate::domain::{QuotaTier, RiskState, TierDashboard};

// ── Sorting ──────────────────────────────────────────────────────

pub fn compare_tiers(a: &TierDashboard, b: &TierDashboard) -> std::cmp::Ordering {
    let (ra, ta) = crate::domain::tier_sort_key(&a.quota.id);
    let (rb, tb) = crate::domain::tier_sort_key(&b.quota.id);
    ra.cmp(&rb)
        .then_with(|| ta.cmp(&tb))
        .then_with(|| a.quota.label.cmp(&b.quota.label))
}

pub fn sorted_tiers(tiers: &[TierDashboard]) -> Vec<&TierDashboard> {
    let mut out: Vec<&TierDashboard> = tiers.iter().collect();
    out.sort_by(|a, b| compare_tiers(a, b));
    out
}

/// Highest-severity tier; ties break on utilization then earliest
/// exhaustion/reset.
pub fn worst_tier(tiers: &[TierDashboard]) -> Option<&TierDashboard> {
    tiers.iter().reduce(|worst, current| {
        use std::cmp::Ordering::*;
        match crate::domain::risk_severity(current.forecast.state)
            .cmp(&crate::domain::risk_severity(worst.forecast.state))
        {
            Greater => current,
            Less => worst,
            Equal => {
                if current.quota.utilization > worst.quota.utilization {
                    current
                } else if current.quota.utilization == worst.quota.utilization
                    && tie_break_time(current) < tie_break_time(worst)
                {
                    current
                } else {
                    worst
                }
            }
        }
    })
}

fn tie_break_time(tier: &TierDashboard) -> i64 {
    tier.forecast
        .exhaustion_at
        .or(tier.quota.resets_at)
        .unwrap_or(i64::MAX)
}

// ── Status semantics ─────────────────────────────────────────────

pub fn risk_severity(state: RiskState) -> i32 {
    crate::domain::risk_severity(state)
}

pub fn status_text(lang: &str, state: RiskState) -> String {
    let key = match state {
        RiskState::Exhausted => "exhausted",
        RiskState::AtRisk => "atRisk",
        RiskState::UnknownReset => "unknownReset",
        RiskState::Learning => "learning",
        RiskState::Safe => "safe",
        RiskState::Error => "error",
    };
    crate::i18n::tr(lang, &format!("status.{key}"))
}

pub fn status_text_short(lang: &str, state: RiskState) -> String {
    let key = match state {
        RiskState::Exhausted => "exhausted",
        RiskState::AtRisk => "atRisk",
        RiskState::UnknownReset => "unknownReset",
        RiskState::Learning => "learning",
        RiskState::Safe => "safe",
        RiskState::Error => "error",
    };
    crate::i18n::tr(lang, &format!("status.short.{key}"))
}

/// Status color by semantic role.
pub fn status_color(theme: &gpui_kit::component::theme::Theme, state: RiskState) -> Hsla {
    match state {
        RiskState::Exhausted | RiskState::Error => theme.danger,
        RiskState::AtRisk => theme.warning,
        RiskState::UnknownReset => theme.muted_foreground,
        RiskState::Learning => theme.info,
        RiskState::Safe => theme.success,
    }
}

/// Color for a utilization figure / progress fill at a given percentage.
pub fn utilization_color(theme: &gpui_kit::component::theme::Theme, tier: &TierDashboard) -> Hsla {
    if tier.forecast.state == RiskState::Exhausted || tier.quota.utilization >= 90.0 {
        theme.danger
    } else if tier.forecast.state == RiskState::AtRisk || tier.quota.utilization >= 70.0 {
        theme.warning
    } else {
        theme.success
    }
}

// ── Formatters ───────────────────────────────────────────────────

fn time_margin_parts(ms: i64) -> Option<(i64, i64)> {
    let diff = ms - chrono::Utc::now().timestamp_millis();
    if diff <= 0 {
        return None;
    }
    let hours = diff / 3_600_000;
    let mins = (diff % 3_600_000) / 60_000;
    Some((hours, mins))
}

pub fn format_time_margin(lang: &str, ms: Option<i64>) -> String {
    let Some(ms) = ms else { return String::new() };
    let Some((h, m)) = time_margin_parts(ms) else {
        return crate::i18n::tr(lang, "format.zeroMinutes");
    };
    if h > 0 {
        crate::i18n::tr_params(
            lang,
            "format.hoursMinutes",
            &[("h", &h.to_string()), ("m", &m.to_string())],
        )
    } else {
        crate::i18n::tr_params(lang, "format.minutes", &[("n", &m.to_string())])
    }
}

pub fn format_time_margin_short(lang: &str, ms: Option<i64>) -> String {
    let Some(ms) = ms else { return String::new() };
    let Some((h, m)) = time_margin_parts(ms) else {
        return crate::i18n::tr(lang, "format.zeroMinutes");
    };
    if h > 0 {
        if m > 0 {
            crate::i18n::tr_params(
                lang,
                "format.hoursMinutesShort",
                &[("h", &h.to_string()), ("m", &m.to_string())],
            )
        } else {
            crate::i18n::tr_params(lang, "format.hoursShort", &[("h", &h.to_string())])
        }
    } else {
        crate::i18n::tr_params(lang, "format.minutesShort", &[("n", &m.to_string())])
    }
}

fn group_number(n: f64) -> String {
    let int_part = n.floor() as i64;
    let frac = n - int_part as f64;
    let mut s = int_part.to_string();
    if frac.abs() >= 0.005 {
        let frac_str = format!("{:.2}", frac);
        s.push_str(&frac_str[1..]); // keep ".xx"
    }
    let mut out = String::new();
    let sign = s.starts_with('-');
    let digits: &str = if sign { &s[1..] } else { &s[..] };
    let (int_digits, frac_suffix) = match digits.find('.') {
        Some(ix) => (&digits[..ix], &digits[ix..]),
        None => (digits, ""),
    };
    if sign {
        out.push('-');
    }
    for (i, b) in int_digits.bytes().enumerate() {
        if i > 0 && (int_digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(b as char);
    }
    out.push_str(frac_suffix);
    out
}

pub fn format_absolute_quota(tier: &QuotaTier) -> Option<String> {
    let used = tier.used?;
    let limit = tier.limit?;
    let unit = tier.unit.clone().unwrap_or_default();
    let unit_suffix = if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    };
    Some(format!(
        "{}/{}{}",
        group_number(used),
        group_number(limit),
        unit_suffix
    ))
}

pub fn utilization_text(tier: &TierDashboard) -> String {
    if tier.quota.unlimited {
        "∞".to_string()
    } else {
        format!("{:.1}%", tier.quota.utilization)
    }
}

/// Primary metadata line for a tier row: exhaustion when at risk, otherwise
/// the reset countdown.
pub fn primary_meta(lang: &str, tier: &TierDashboard, compact: bool) -> String {
    if tier.quota.unlimited {
        return crate::i18n::tr(lang, "status.safe");
    }
    match tier.forecast.state {
        RiskState::Exhausted => crate::i18n::tr(lang, "status.exhausted"),
        RiskState::AtRisk => {
            if tier.forecast.exhaustion_at.is_some() {
                let time = if compact {
                    format_time_margin_short(lang, tier.forecast.exhaustion_at)
                } else {
                    format_time_margin(lang, tier.forecast.exhaustion_at)
                };
                crate::i18n::tr_params(lang, "format.exhaustsIn", &[("time", &time)])
            } else {
                reset_text(lang, tier, compact)
            }
        }
        _ => reset_text(lang, tier, compact),
    }
}

fn reset_text(lang: &str, tier: &TierDashboard, compact: bool) -> String {
    if tier.quota.resets_at.is_some() {
        let time = if compact {
            format_time_margin_short(lang, tier.quota.resets_at)
        } else {
            format_time_margin(lang, tier.quota.resets_at)
        };
        crate::i18n::tr_params(lang, "format.resetsIn", &[("time", &time)])
    } else {
        crate::i18n::tr(lang, "format.resetUnknown")
    }
}

pub fn rate_text(lang: &str, tier: &TierDashboard) -> String {
    if tier.forecast.state == RiskState::Learning {
        crate::i18n::tr(lang, "format.rateAnalyzing")
    } else {
        crate::i18n::tr_params(
            lang,
            "format.ratePerHour",
            &[("rate", &format!("{:.1}", tier.forecast.rate_per_hour))],
        )
    }
}

// ── Row rendering ────────────────────────────────────────────────

/// Visual content of one quota tier row (label, utilization, progress bar,
/// meta lines). `compact` tightens it for the tray panel. Interactive
/// wrapping (click-to-select) is the caller's job.
pub fn tier_content(theme: &Theme, tier: &TierDashboard, lang: &str, compact: bool) -> AnyElement {
    let quota = &tier.quota;
    let pct_color = utilization_color(theme, tier);
    let meta = primary_meta(lang, tier, compact);
    let meta_color = match tier.forecast.state {
        RiskState::Exhausted | RiskState::Error => theme.colors.danger,
        RiskState::AtRisk => theme.colors.warning,
        _ => theme.colors.muted_foreground,
    };

    let pct_text = if quota.unlimited {
        crate::i18n::tr(lang, "overview.unlimitedQuota")
    } else if compact {
        format!("{:.0}%", quota.utilization)
    } else {
        format!("{:.1}%", quota.utilization)
    };

    // Compact rows (tray) run one size step down from the main dashboard.
    let headline_text = |d: Div| if compact { d.text_xs() } else { d.text_sm() };
    let meta_text = |d: Div| if compact { d.text_xs() } else { d.text_xs() };

    let headline = h_flex()
        .w_full()
        .justify_between()
        .gap_2()
        .child(
            headline_text(div().min_w_0().flex_1())
                .text_color(theme.colors.foreground)
                .truncate()
                .child(quota.label.clone()),
        )
        .child(
            headline_text(div().flex_shrink_0())
                .font_weight(FontWeight::BOLD)
                .text_color(if quota.unlimited {
                    theme.colors.success
                } else {
                    pct_color
                })
                .child(pct_text),
        );

    let bar = if quota.unlimited {
        div().into_any_element()
    } else {
        Progress::new(format!("bar-{}", quota.id))
            .value(quota.utilization.clamp(0.0, 100.0) as f32)
            .color(pct_color)
            .into_any_element()
    };

    let mut col = v_flex().w_full().gap_1().child(headline).child(bar);

    if !compact {
        if let Some(absolute) = format_absolute_quota(quota) {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(absolute),
            );
        }
    }

    col = col.child(
        h_flex()
            .w_full()
            .justify_between()
            .gap_2()
            .child(
                meta_text(div().min_w_0().flex_1())
                    .text_color(meta_color)
                    .truncate()
                    .child(meta),
            )
            .child(
                meta_text(div().flex_shrink_0())
                    .text_color(theme.colors.muted_foreground.opacity(0.8))
                    .child(rate_text(lang, tier)),
            ),
    );

    col.into_any_element()
}
