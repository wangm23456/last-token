use crate::domain::{RiskState, TierForecast, HistoryPoint};

pub fn compute_forecast(
    unlimited: bool,
    current_utilization: f64,
    resets_at: Option<i64>,
    snapshots: &[HistoryPoint], // Assumed sorted chronologically
    now_ms: i64,
    polling_interval_mins: i64,
) -> TierForecast {
    // 1. safe immediately for quota.unlimited == true
    if unlimited {
        return TierForecast {
            state: RiskState::Safe,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: 0.0,
            exhaustion_at: None,
            sample_count: 0,
            observation_minutes: 0,
        };
    }

    // 2. exhausted when current utilization >= 99.9%
    if current_utilization >= 99.9 {
        return TierForecast {
            state: RiskState::Exhausted,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: 100.0,
            exhaustion_at: Some(now_ms), // exhaustion is now
            sample_count: snapshots.len(),
            observation_minutes: if snapshots.is_empty() { 0 } else { (snapshots.last().unwrap().sampled_at - snapshots.first().unwrap().sampled_at) / 60000 },
        };
    }

    // 3. unknown_reset when resetsAt is absent or not in the future
    let resets_at_val = match resets_at {
        Some(r) if r > now_ms => r,
        _ => {
            return TierForecast {
                state: RiskState::UnknownReset,
                rate_per_hour: 0.0,
                projected_utilization_at_reset: current_utilization,
                exhaustion_at: None,
                sample_count: snapshots.len(),
                observation_minutes: if snapshots.is_empty() { 0 } else { (snapshots.last().unwrap().sampled_at - snapshots.first().unwrap().sampled_at) / 60000 },
            };
        }
    };

    // 4. Segment historical data.
    // Load at most the last 24 hours of successful snapshots ordered by time.
    let limit_ms = now_ms - 24 * 60 * 60 * 1000;
    let relevant_snapshots: Vec<&HistoryPoint> = snapshots
        .iter()
        .filter(|s| s.sampled_at >= limit_ms)
        .collect();

    // Scan backward to find the break point
    let mut break_index = 0;
    if !relevant_snapshots.is_empty() {
        for i in (1..relevant_snapshots.len()).rev() {
            let prev = relevant_snapshots[i - 1];
            let curr = relevant_snapshots[i];
            
            // Check drop in utilization > 2%
            let drop = prev.utilization - curr.utilization;
            if drop > 2.0 {
                break_index = i;
                break;
            }
            
            // Check resetsAt difference > 2 * polling_interval_mins
            if let (Some(prev_r), Some(curr_r)) = (prev.resets_at, curr.resets_at) {
                let diff_ms = (prev_r - curr_r).abs();
                let threshold_ms = polling_interval_mins * 2 * 60 * 1000;
                if diff_ms > threshold_ms {
                    break_index = i;
                    break;
                }
            }
        }
    }

    let segment = &relevant_snapshots[break_index..];
    
    // Check requirements: at least 3 samples spanning at least 15 minutes
    if segment.len() < 3 {
        return TierForecast {
            state: RiskState::Learning,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: current_utilization,
            exhaustion_at: None,
            sample_count: segment.len(),
            observation_minutes: if segment.is_empty() { 0 } else { (segment.last().unwrap().sampled_at - segment.first().unwrap().sampled_at) / 60000 },
        };
    }
    
    let span_ms = segment.last().unwrap().sampled_at - segment.first().unwrap().sampled_at;
    if span_ms < 15 * 60 * 1000 {
        return TierForecast {
            state: RiskState::Learning,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: current_utilization,
            exhaustion_at: None,
            sample_count: segment.len(),
            observation_minutes: span_ms / 60000,
        };
    }

    // Fit ordinary least squares: utilization % against elapsed hours
    let t0 = segment.first().unwrap().sampled_at as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let n = segment.len() as f64;

    for s in segment {
        let x = (s.sampled_at as f64 - t0) / 3600000.0; // hours elapsed since t0
        let y = s.utilization;
        sum_x += x;
        sum_y += y;
        sum_xx += x * x;
        sum_xy += x * y;
    }

    let denominator = n * sum_xx - sum_x * sum_x;
    let slope = if denominator.abs() < 1e-9 {
        0.0
    } else {
        (n * sum_xy - sum_x * sum_y) / denominator
    };

    // Clamp negative slope to 0
    let mut rate_per_hour = slope.max(0.0);

    // If the observed range is below 1 percentage point or the fitted slope is at most 0.05%/hour, treat pace as zero
    let min_y = segment.iter().map(|s| s.utilization).fold(f64::INFINITY, f64::min);
    let max_y = segment.iter().map(|s| s.utilization).fold(f64::NEG_INFINITY, f64::max);
    let range_y = max_y - min_y;

    if range_y < 1.0 || rate_per_hour <= 0.05 {
        rate_per_hour = 0.0;
    }

    let observation_minutes = span_ms / 60000;

    if rate_per_hour == 0.0 {
        return TierForecast {
            state: RiskState::Safe,
            rate_per_hour: 0.0,
            projected_utilization_at_reset: current_utilization,
            exhaustion_at: None,
            sample_count: segment.len(),
            observation_minutes,
        };
    }

    // Calculations
    let hours_to_reset = (resets_at_val - now_ms) as f64 / 3600000.0;
    let projected_utilization_at_reset = (current_utilization + rate_per_hour * hours_to_reset).min(100.0);
    
    let hours_to_exhaustion = (100.0 - current_utilization) / rate_per_hour;
    let exhaustion_at = now_ms + (hours_to_exhaustion * 3600000.0).round() as i64;

    let state = if exhaustion_at <= resets_at_val {
        RiskState::AtRisk
    } else {
        RiskState::Safe
    };

    TierForecast {
        state,
        rate_per_hour,
        projected_utilization_at_reset,
        exhaustion_at: Some(exhaustion_at),
        sample_count: segment.len(),
        observation_minutes,
    }
}
