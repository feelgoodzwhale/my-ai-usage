use crate::collector::statusline::StatuslineSnapshot;
use crate::collector::UsageRecord;
use crate::config::{AgyQuotaMode, AppConfig};
use chrono::{Datelike, Duration, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FIVE_HOURS_SECS: i64 = 5 * 3600;
const SEVEN_DAYS_SECS: i64 = 7 * 86_400;
const THIRTY_DAYS_SECS: i64 = 30 * 86_400;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaWindowStatus {
    pub used_pct: f64,
    pub used_tokens: u64,
    pub limit_tokens: Option<u64>,
    pub resets_at: Option<i64>,
    pub source: String, // "official" | "local_block"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenSummary {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub active_tokens: u64,
    pub total_tokens: u64,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelUsageItem {
    pub model: String,
    pub active_tokens: u64,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub share_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RangeStats {
    pub tokens: TokenSummary,
    pub models: Vec<ModelUsageItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrendBucket {
    pub label: String,
    pub timestamp: i64,
    pub active_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderUsageReport {
    pub id: String,
    pub name: String,
    pub plan_label: String,
    pub active_model: Option<String>,
    pub five_hour: QuotaWindowStatus,
    pub weekly: QuotaWindowStatus,
    pub ranges: ProviderRanges,
    pub hourly_24h: Vec<TrendBucket>,
    pub daily_30d: Vec<TrendBucket>,
    pub last_updated: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderRanges {
    pub five_hour: RangeStats,
    pub today: RangeStats,
    pub seven_day: RangeStats,
    pub thirty_day: RangeStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullUsagePayload {
    pub claude: ProviderUsageReport,
    pub agy: ProviderUsageReport,
    pub config: AppConfig,
    pub generated_at: i64,
}

pub fn quota_tokens_for_record(rec: &UsageRecord, mode: AgyQuotaMode) -> u64 {
    match mode {
        AgyQuotaMode::ActiveOnly => rec.input_tokens + rec.output_tokens,
        AgyQuotaMode::WeightedCache => {
            rec.input_tokens + rec.output_tokens + (rec.cache_read_tokens / 10)
        }
        AgyQuotaMode::IncludeAllCache => {
            rec.input_tokens + rec.output_tokens + rec.cache_read_tokens + rec.cache_write_tokens
        }
    }
}

/// Computes a Session-Anchored block (e.g. 5 hours or 7 days).
/// First message starts `[block_start, block_start + window_secs]`.
/// Messages inside the block accumulate tokens; the first message at or after `block_end` opens a new block.
pub fn compute_session_block(
    records: &[UsageRecord],
    now: i64,
    window_secs: i64,
    lookback_secs: i64,
    limit_tokens: u64,
    mode: AgyQuotaMode,
) -> (QuotaWindowStatus, i64) {
    let min_ts = now - lookback_secs;
    let mut block_start: Option<i64> = None;
    let mut block_end: i64 = 0;
    let mut block_tokens: u64 = 0;

    for rec in records.iter().filter(|r| r.timestamp >= min_ts && r.timestamp <= now + 60) {
        let t = quota_tokens_for_record(rec, mode);
        match block_start {
            None => {
                block_start = Some(rec.timestamp);
                block_end = rec.timestamp + window_secs;
                block_tokens = t;
            }
            Some(_) => {
                if rec.timestamp < block_end {
                    block_tokens = block_tokens.saturating_add(t);
                } else {
                    block_start = Some(rec.timestamp);
                    block_end = rec.timestamp + window_secs;
                    block_tokens = t;
                }
            }
        }
    }

    if let Some(start) = block_start {
        if now < block_end {
            let pct = if limit_tokens > 0 {
                ((block_tokens as f64 / limit_tokens as f64) * 100.0).clamp(0.0, 999.0)
            } else {
                0.0
            };
            return (
                QuotaWindowStatus {
                    used_pct: pct,
                    used_tokens: block_tokens,
                    limit_tokens: Some(limit_tokens),
                    resets_at: Some(block_end),
                    source: "local_block".to_string(),
                },
                start,
            );
        }
    }

    (
        QuotaWindowStatus {
            used_pct: 0.0,
            used_tokens: 0,
            limit_tokens: Some(limit_tokens),
            resets_at: None,
            source: "local_block".to_string(),
        },
        now - window_secs,
    )
}

pub fn build_provider_report(
    id: &str,
    name: &str,
    records: &[UsageRecord],
    snapshot: Option<&StatuslineSnapshot>,
    five_hour_limit: u64,
    weekly_limit: u64,
    quota_mode: AgyQuotaMode,
    now: i64,
) -> ProviderUsageReport {
    let (local_5h, block_5h_start) = compute_session_block(
        records,
        now,
        FIVE_HOURS_SECS,
        48 * 3600,
        five_hour_limit,
        quota_mode,
    );

    let (local_7d, _block_7d_start) = compute_session_block(
        records,
        now,
        SEVEN_DAYS_SECS,
        35 * 86_400,
        weekly_limit,
        quota_mode,
    );

    // Resolve 5-hour quota (prefer official statusLine snapshot if valid)
    let five_hour = resolve_hybrid_window(
        snapshot.and_then(|s| s.five_hour.as_ref()),
        snapshot.and_then(|s| s.updated_at),
        &local_5h,
        records,
        five_hour_limit,
        quota_mode,
        now,
    );

    // Resolve Weekly quota
    let weekly = resolve_hybrid_window(
        snapshot.and_then(|s| s.seven_day.as_ref()),
        snapshot.and_then(|s| s.updated_at),
        &local_7d,
        records,
        weekly_limit,
        quota_mode,
        now,
    );

    let local_now = Local
        .timestamp_opt(now, 0)
        .single()
        .unwrap_or_else(Local::now);
    let today_start = local_now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|ndt| Local.from_local_datetime(&ndt).single())
        .map(|dt| dt.timestamp())
        .unwrap_or(now - 86_400);

    let five_h_stats_start = if five_hour.resets_at.is_some() {
        five_hour
            .resets_at
            .map(|r| r - FIVE_HOURS_SECS)
            .unwrap_or(block_5h_start)
    } else {
        now - FIVE_HOURS_SECS
    };

    let ranges = ProviderRanges {
        five_hour: aggregate_range(records, five_h_stats_start, now + 60),
        today: aggregate_range(records, today_start, now + 60),
        seven_day: aggregate_range(records, now - SEVEN_DAYS_SECS, now + 60),
        thirty_day: aggregate_range(records, now - THIRTY_DAYS_SECS, now + 60),
    };

    let hourly_24h = build_hourly_buckets(records, now);
    let daily_30d = build_daily_buckets(records, now, 30);

    let active_model = snapshot
        .and_then(|s| s.model_name.clone())
        .or_else(|| records.last().map(|r| r.model.clone()));

    let plan_label = if let Some(tier) = snapshot.and_then(|s| s.plan_tier.as_ref()) {
        tier.clone()
    } else if five_hour.source == "official" {
        "Official Rate Limits".to_string()
    } else {
        "Local Quota Block".to_string()
    };

    ProviderUsageReport {
        id: id.to_string(),
        name: name.to_string(),
        plan_label,
        active_model,
        five_hour,
        weekly,
        ranges,
        hourly_24h,
        daily_30d,
        last_updated: now,
    }
}

fn resolve_hybrid_window(
    official: Option<&crate::collector::statusline::RateLimitEntry>,
    snap_updated_at: Option<i64>,
    local_fallback: &QuotaWindowStatus,
    records: &[UsageRecord],
    limit_tokens: u64,
    mode: AgyQuotaMode,
    now: i64,
) -> QuotaWindowStatus {
    if let Some(off) = official {
        if let Some(reset_ts) = off.resets_at {
            if now < reset_ts {
                return QuotaWindowStatus {
                    used_pct: off.used_percentage.clamp(0.0, 999.0),
                    used_tokens: local_fallback.used_tokens,
                    limit_tokens: None,
                    resets_at: Some(reset_ts),
                    source: "official".to_string(),
                };
            } else {
                // Official window already passed reset_ts!
                // Check if new local messages happened after reset_ts without refreshing statusline.
                let post_reset_tokens: u64 = records
                    .iter()
                    .filter(|r| r.timestamp >= reset_ts)
                    .map(|r| quota_tokens_for_record(r, mode))
                    .sum();
                if post_reset_tokens == 0 {
                    return QuotaWindowStatus {
                        used_pct: 0.0,
                        used_tokens: 0,
                        limit_tokens: None,
                        resets_at: None,
                        source: "official".to_string(),
                    };
                }
            }
        } else if let Some(updated) = snap_updated_at {
            if now - updated < FIVE_HOURS_SECS {
                return QuotaWindowStatus {
                    used_pct: off.used_percentage.clamp(0.0, 999.0),
                    used_tokens: local_fallback.used_tokens,
                    limit_tokens: Some(limit_tokens),
                    resets_at: local_fallback.resets_at,
                    source: "official".to_string(),
                };
            }
        }
    }

    local_fallback.clone()
}

fn aggregate_range(records: &[UsageRecord], start_ts: i64, end_ts: i64) -> RangeStats {
    let mut summary = TokenSummary::default();
    let mut by_model: HashMap<String, ModelUsageItem> = HashMap::new();

    for rec in records
        .iter()
        .filter(|r| r.timestamp >= start_ts && r.timestamp <= end_ts)
    {
        let active = rec.input_tokens + rec.output_tokens;
        let total = rec.total_tokens();

        summary.input_tokens += rec.input_tokens;
        summary.output_tokens += rec.output_tokens;
        summary.cache_read_tokens += rec.cache_read_tokens;
        summary.cache_write_tokens += rec.cache_write_tokens;
        summary.active_tokens += active;
        summary.total_tokens += total;
        summary.message_count += 1;

        let entry = by_model
            .entry(rec.model.clone())
            .or_insert_with(|| ModelUsageItem {
                model: rec.model.clone(),
                ..Default::default()
            });
        entry.input_tokens += rec.input_tokens;
        entry.output_tokens += rec.output_tokens;
        entry.cache_read_tokens += rec.cache_read_tokens;
        entry.cache_write_tokens += rec.cache_write_tokens;
        entry.active_tokens += active;
        entry.total_tokens += total;
    }

    let mut models: Vec<ModelUsageItem> = by_model.into_values().collect();
    models.sort_by(|a, b| {
        b.active_tokens
            .cmp(&a.active_tokens)
            .then_with(|| b.total_tokens.cmp(&a.total_tokens))
    });

    let denom = if summary.active_tokens > 0 {
        summary.active_tokens as f64
    } else {
        summary.total_tokens.max(1) as f64
    };

    for m in &mut models {
        let num = if summary.active_tokens > 0 {
            m.active_tokens as f64
        } else {
            m.total_tokens as f64
        };
        m.share_pct = (num / denom) * 100.0;
    }

    RangeStats {
        tokens: summary,
        models,
    }
}

fn build_hourly_buckets(records: &[UsageRecord], now: i64) -> Vec<TrendBucket> {
    let local_now = Local
        .timestamp_opt(now, 0)
        .single()
        .unwrap_or_else(Local::now);
    let current_hour = local_now
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(local_now);

    let first_hour = current_hour - Duration::hours(23);
    let first_ts = first_hour.timestamp();

    let mut buckets: Vec<TrendBucket> = (0..24)
        .map(|i| {
            let dt = first_hour + Duration::hours(i);
            TrendBucket {
                label: format!("{:02}:00", dt.hour()),
                timestamp: dt.timestamp(),
                ..Default::default()
            }
        })
        .collect();

    for rec in records.iter().filter(|r| r.timestamp >= first_ts) {
        let idx = ((rec.timestamp - first_ts) / 3600) as isize;
        if (0..24).contains(&idx) {
            let b = &mut buckets[idx as usize];
            b.input_tokens += rec.input_tokens;
            b.output_tokens += rec.output_tokens;
            b.cache_read_tokens += rec.cache_read_tokens;
            b.cache_write_tokens += rec.cache_write_tokens;
            b.active_tokens += rec.input_tokens + rec.output_tokens;
        }
    }

    buckets
}

fn build_daily_buckets(records: &[UsageRecord], now: i64, days: i64) -> Vec<TrendBucket> {
    let local_now = Local
        .timestamp_opt(now, 0)
        .single()
        .unwrap_or_else(Local::now);
    let today_date = local_now.date_naive();
    let start_date = today_date - Duration::days(days - 1);

    let mut date_to_idx: HashMap<chrono::NaiveDate, usize> = HashMap::new();
    let mut buckets = Vec::with_capacity(days as usize);

    for i in 0..days {
        let d = start_date + Duration::days(i);
        let ts = d
            .and_hms_opt(0, 0, 0)
            .and_then(|ndt| Local.from_local_datetime(&ndt).single())
            .map(|dt| dt.timestamp())
            .unwrap_or(now);
        date_to_idx.insert(d, i as usize);
        buckets.push(TrendBucket {
            label: format!("{:02}/{:02}", d.month(), d.day()),
            timestamp: ts,
            ..Default::default()
        });
    }

    let cutoff_ts = buckets.first().map(|b| b.timestamp).unwrap_or(now);
    for rec in records.iter().filter(|r| r.timestamp >= cutoff_ts - 86_400) {
        if let Some(dt) = Local.timestamp_opt(rec.timestamp, 0).single() {
            if let Some(&idx) = date_to_idx.get(&dt.date_naive()) {
                let b = &mut buckets[idx];
                b.input_tokens += rec.input_tokens;
                b.output_tokens += rec.output_tokens;
                b.cache_read_tokens += rec.cache_read_tokens;
                b.cache_write_tokens += rec.cache_write_tokens;
                b.active_tokens += rec.input_tokens + rec.output_tokens;
            }
        }
    }

    buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_rec(ts: i64, inp: u64, out: u64, cr: u64) -> UsageRecord {
        UsageRecord {
            timestamp: ts,
            model: "Gemini 4 Argon".to_string(),
            input_tokens: inp,
            output_tokens: out,
            cache_read_tokens: cr,
            cache_write_tokens: 0,
        }
    }

    #[test]
    fn test_session_anchored_5h_block_accumulates_and_resets() {
        let t0 = 1_700_000_000;
        let records = vec![
            mk_rec(t0, 50_000, 50_000, 300_000),        // 100k active
            mk_rec(t0 + 3600, 30_000, 20_000, 400_000), // +50k active -> 150k
        ];

        // 2 hours into the 5h block: active = 150k / 500k = 30%, resets at t0 + 18000
        let (status, _) = compute_session_block(
            &records,
            t0 + 7200,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        assert!((status.used_pct - 30.0).abs() < 0.01);
        assert_eq!(status.used_tokens, 150_000);
        assert_eq!(status.resets_at, Some(t0 + FIVE_HOURS_SECS));

        // After 5h + 10s with no new messages: resets to 0%!
        let (expired, _) = compute_session_block(
            &records,
            t0 + FIVE_HOURS_SECS + 10,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        assert_eq!(expired.used_pct, 0.0);
        assert_eq!(expired.resets_at, None);
    }

    #[test]
    fn test_new_block_opens_after_previous_5h_expires() {
        let t0 = 1_700_000_000;
        let t1 = t0 + FIVE_HOURS_SECS + 600;
        let records = vec![
            mk_rec(t0, 200_000, 50_000, 0),
            mk_rec(t1, 40_000, 10_000, 0), // New block: 50k / 500k = 10%
        ];

        let (status, _) = compute_session_block(
            &records,
            t1 + 300,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        assert!((status.used_pct - 10.0).abs() < 0.01);
        assert_eq!(status.resets_at, Some(t1 + FIVE_HOURS_SECS));
    }
}
