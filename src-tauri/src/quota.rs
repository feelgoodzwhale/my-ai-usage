use crate::collector::statusline::{RateLimitEntry, StatuslineSnapshot, SubQuotaBucket};
use crate::collector::UsageRecord;
use crate::config::{AgyQuotaMode, AppConfig};
use chrono::{Datelike, Duration, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FIVE_HOURS_SECS: i64 = 5 * 3600;
const SEVEN_DAYS_SECS: i64 = 7 * 86_400;
const THIRTY_DAYS_SECS: i64 = 30 * 86_400;
const AGY_MAX_STALE_SECS: i64 = 6 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaWindowStatus {
    pub used_pct: f64,
    pub remaining_pct: f64,
    pub used_tokens: u64,
    pub limit_tokens: Option<u64>,
    pub resets_at: Option<i64>,
    pub source: String, // "official" | "official+live" | "calibrated_cost" | "local_block"
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
    pub quota_groups: Vec<SubQuotaBucket>,
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

/// Computes Anthropic API dollar-equivalent cost for a single Claude UsageRecord,
/// properly accounting for Opus vs Sonnet vs Haiku pricing as well as
/// `cache_read_input_tokens` (0.1x) and `cache_creation_input_tokens` (1.25x).
pub fn compute_claude_record_cost_usd(rec: &UsageRecord) -> f64 {
    let m = rec.model.to_ascii_lowercase();
    let (base_in, base_out) = if m.contains("opus") {
        if m.contains("opus 3")
            || m.contains("opus-3")
            || m.contains("opus 4.0")
            || m.contains("opus-4-0")
            || m.contains("opus 4.1")
            || m.contains("opus-4-1")
        {
            (15.0, 75.0)
        } else {
            (5.0, 25.0)
        }
    } else if m.contains("haiku") {
        (1.0, 5.0)
    } else {
        (3.0, 15.0)
    };

    let cache_read_rate = base_in * 0.10;
    let cache_write_rate = base_in * 1.25;

    ((rec.input_tokens as f64) * base_in
        + (rec.output_tokens as f64) * base_out
        + (rec.cache_read_tokens as f64) * cache_read_rate
        + (rec.cache_write_tokens as f64) * cache_write_rate)
        / 1_000_000.0
}

/// Computes Claude's 5-hour session window with hour-floored start boundaries
/// (`HH:00:00` -> `HH+5:00:00`, matching Anthropic's 5h reset clock) and API cost budget.
pub fn compute_claude_5h_cost_window(
    records: &[UsageRecord],
    now: i64,
    budget_usd: f64,
) -> (QuotaWindowStatus, i64) {
    let min_ts = now - 48 * 3600;
    let mut block_start: Option<i64> = None;
    let mut block_end: i64 = 0;
    let mut block_cost_usd: f64 = 0.0;
    let mut block_active_tokens: u64 = 0;

    for rec in records
        .iter()
        .filter(|r| r.timestamp >= min_ts && r.timestamp <= now + 60)
    {
        let cost = compute_claude_record_cost_usd(rec);
        let active_tok = rec.input_tokens + rec.output_tokens;
        let floored_hour = rec.timestamp.div_euclid(3600) * 3600;

        match block_start {
            None => {
                block_start = Some(floored_hour);
                block_end = floored_hour + FIVE_HOURS_SECS;
                block_cost_usd = cost;
                block_active_tokens = active_tok;
            }
            Some(_) => {
                if rec.timestamp < block_end {
                    block_cost_usd += cost;
                    block_active_tokens = block_active_tokens.saturating_add(active_tok);
                } else {
                    block_start = Some(floored_hour);
                    block_end = floored_hour + FIVE_HOURS_SECS;
                    block_cost_usd = cost;
                    block_active_tokens = active_tok;
                }
            }
        }
    }

    if let Some(start) = block_start {
        if now < block_end {
            let safe_budget = budget_usd.max(1.0);
            let used_pct = ((block_cost_usd / safe_budget) * 100.0).clamp(0.0, 100.0);
            return (
                QuotaWindowStatus {
                    used_pct,
                    remaining_pct: (100.0 - used_pct).clamp(0.0, 100.0),
                    used_tokens: block_active_tokens,
                    limit_tokens: None,
                    resets_at: Some(block_end),
                    source: "calibrated_cost".to_string(),
                },
                start,
            );
        }
    }

    (
        QuotaWindowStatus {
            used_pct: 0.0,
            remaining_pct: 100.0,
            used_tokens: 0,
            limit_tokens: None,
            resets_at: None,
            source: "calibrated_cost".to_string(),
        },
        now - FIVE_HOURS_SECS,
    )
}

/// Computes Claude's 7-day weekly window anchored to Anthropic's fixed weekly reset cadence
/// (e.g. Friday 04:00 AM local time from `~/.claude.json` `seven_day.resets_at`).
pub fn compute_claude_weekly_cost_window(
    records: &[UsageRecord],
    now: i64,
    budget_usd: f64,
    weekly_reset_anchor: Option<i64>,
) -> QuotaWindowStatus {
    let (window_start, resets_at) = if let Some(anchor_ts) = weekly_reset_anchor {
        let window_end = if anchor_ts > now {
            let back = (anchor_ts - now - 1) / SEVEN_DAYS_SECS;
            anchor_ts - back * SEVEN_DAYS_SECS
        } else {
            let periods = ((now - anchor_ts) / SEVEN_DAYS_SECS) + 1;
            anchor_ts + periods * SEVEN_DAYS_SECS
        };
        (window_end - SEVEN_DAYS_SECS, Some(window_end))
    } else {
        (now - SEVEN_DAYS_SECS, None)
    };

    let mut total_cost_usd = 0.0;
    let mut active_tokens: u64 = 0;
    for rec in records
        .iter()
        .filter(|r| r.timestamp >= window_start && r.timestamp <= now + 60)
    {
        total_cost_usd += compute_claude_record_cost_usd(rec);
        active_tokens = active_tokens.saturating_add(rec.input_tokens + rec.output_tokens);
    }

    let safe_budget = budget_usd.max(5.0);
    let used_pct = ((total_cost_usd / safe_budget) * 100.0).clamp(0.0, 100.0);

    QuotaWindowStatus {
        used_pct,
        remaining_pct: (100.0 - used_pct).clamp(0.0, 100.0),
        used_tokens: active_tokens,
        limit_tokens: None,
        resets_at,
        source: "calibrated_cost".to_string(),
    }
}

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

    for rec in records
        .iter()
        .filter(|r| r.timestamp >= min_ts && r.timestamp <= now + 60)
    {
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
                    remaining_pct: (100.0 - pct).clamp(0.0, 100.0),
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
            remaining_pct: 100.0,
            used_tokens: 0,
            limit_tokens: Some(limit_tokens),
            resets_at: None,
            source: "local_block".to_string(),
        },
        now - window_secs,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_provider_report(
    id: &str,
    name: &str,
    records: &[UsageRecord],
    snapshot: Option<&StatuslineSnapshot>,
    latest_transcript_model: Option<&(i64, String)>,
    config: &AppConfig,
    now: i64,
) -> ProviderUsageReport {
    let is_claude = id == "claude";
    let quota_mode = if is_claude {
        AgyQuotaMode::ActiveOnly
    } else {
        config.agy_quota_mode
    };

    let (local_5h, block_5h_start) = if is_claude {
        compute_claude_5h_cost_window(records, now, config.claude_five_hour_budget_usd)
    } else {
        compute_session_block(
            records,
            now,
            FIVE_HOURS_SECS,
            48 * 3600,
            config.agy_five_hour_limit,
            quota_mode,
        )
    };

    let local_7d = if is_claude {
        compute_claude_weekly_cost_window(
            records,
            now,
            config.claude_weekly_budget_usd,
            snapshot.and_then(|s| s.weekly_reset_anchor),
        )
    } else {
        compute_session_block(
            records,
            now,
            SEVEN_DAYS_SECS,
            35 * 86_400,
            config.agy_weekly_limit,
            quota_mode,
        )
        .0
    };

    let snap_quota_ts = snapshot.and_then(|s| s.quota_updated_at);

    let five_hour = resolve_hybrid_window(
        id,
        snapshot.and_then(|s| s.five_hour.as_ref()),
        snap_quota_ts,
        &local_5h,
        records,
        quota_mode,
        if is_claude {
            Some(config.claude_five_hour_budget_usd)
        } else {
            None
        },
        now,
    );

    let weekly = resolve_hybrid_window(
        id,
        snapshot.and_then(|s| s.seven_day.as_ref()),
        snap_quota_ts,
        &local_7d,
        records,
        quota_mode,
        if is_claude {
            Some(config.claude_weekly_budget_usd)
        } else {
            None
        },
        now,
    );

    let quota_groups: Vec<SubQuotaBucket> = snapshot
        .map(|s| {
            s.quota_buckets
                .iter()
                .filter_map(|b| {
                    if let Some(rts) = b.resets_at {
                        if now >= rts {
                            if id == "claude" {
                                return None;
                            }
                            let mut reset_b = b.clone();
                            reset_b.used_pct = 0.0;
                            reset_b.remaining_pct = 100.0;
                            reset_b.resets_at = None;
                            return Some(reset_b);
                        }
                    }
                    Some(b.clone())
                })
                .collect()
        })
        .unwrap_or_default();

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

    let five_h_stats_start = if let Some(r) = five_hour.resets_at {
        r - FIVE_HOURS_SECS
    } else if five_hour.source.starts_with("official") {
        now - FIVE_HOURS_SECS
    } else {
        block_5h_start
    };

    let ranges = ProviderRanges {
        five_hour: aggregate_range(records, five_h_stats_start, now + 60),
        today: aggregate_range(records, today_start, now + 60),
        seven_day: aggregate_range(records, now - SEVEN_DAYS_SECS, now + 60),
        thirty_day: aggregate_range(records, now - THIRTY_DAYS_SECS, now + 60),
    };

    let hourly_24h = build_hourly_buckets(records, now);
    let daily_30d = build_daily_buckets(records, now, 30);

    let active_model = resolve_active_model(id, records, snapshot, latest_transcript_model);

    let plan_label = if five_hour.source.starts_with("official")
        || weekly.source.starts_with("official")
    {
        snapshot
            .and_then(|s| s.plan_tier.clone())
            .unwrap_or_else(|| "Official Quota".to_string())
    } else if is_claude {
        "Claude (Cost Calibrated)".to_string()
    } else {
        "Local 5h Block".to_string()
    };

    let last_updated = snap_quota_ts
        .or_else(|| records.last().map(|r| r.timestamp))
        .unwrap_or(now);

    ProviderUsageReport {
        id: id.to_string(),
        name: name.to_string(),
        plan_label,
        active_model,
        five_hour,
        weekly,
        quota_groups,
        ranges,
        hourly_24h,
        daily_30d,
        last_updated,
    }
}

fn resolve_active_model(
    id: &str,
    records: &[UsageRecord],
    snapshot: Option<&StatuslineSnapshot>,
    latest_transcript_model: Option<&(i64, String)>,
) -> Option<String> {
    let snap_model = snapshot.and_then(|s| {
        let m = s.model_name.as_ref()?;
        let has_active_ctx = s.context_used_pct.unwrap_or(0.0) > 0.0;
        let ts = if id == "claude" {
            s.quota_updated_at.unwrap_or(0)
        } else if has_active_ctx {
            s.updated_at.unwrap_or(0)
        } else {
            // Idle zero-token CLI probe: yield to real Desktop/CLI transcript model if available
            0
        };
        Some((ts, m.clone()))
    });

    let rec_model = latest_transcript_model
        .cloned()
        .or_else(|| records.last().map(|r| (r.timestamp, r.model.clone())));

    match (snap_model, rec_model) {
        (Some((sts, sm)), Some((rts, rm))) => {
            if rts > sts {
                Some(rm)
            } else {
                Some(sm)
            }
        }
        (Some((_, sm)), None) => Some(sm),
        (None, Some((_, rm))) => Some(rm),
        (None, None) => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_hybrid_window(
    provider_id: &str,
    official: Option<&RateLimitEntry>,
    snap_quota_updated_at: Option<i64>,
    local_fallback: &QuotaWindowStatus,
    records: &[UsageRecord],
    mode: AgyQuotaMode,
    claude_budget_usd: Option<f64>,
    now: i64,
) -> QuotaWindowStatus {
    if let Some(off) = official {
        // Guard against multi-day-old AGY rolling snapshots if live probe hasn't run yet
        if provider_id == "agy" {
            if let Some(qts) = snap_quota_updated_at {
                if now - qts > AGY_MAX_STALE_SECS {
                    let newer_tokens: u64 = records
                        .iter()
                        .filter(|r| r.timestamp > qts + 30)
                        .map(|r| quota_tokens_for_record(r, mode))
                        .sum();
                    if newer_tokens > 0 && off.resets_at.is_some_and(|rts| rts - now > FIVE_HOURS_SECS) {
                        return local_fallback.clone();
                    }
                }
            }
        }

        if let Some(reset_ts) = off.resets_at {
            if now < reset_ts {
                let mut used = off.used_percentage.clamp(0.0, 999.0);
                let mut source = "official".to_string();

                // If Claude had new local Desktop turns AFTER the last official CLI snapshot,
                // add the incremental API-cost delta onto the official checkpoint!
                if provider_id == "claude" {
                    if let (Some(qts), Some(budget)) = (snap_quota_updated_at, claude_budget_usd) {
                        if qts > 0 {
                            let post_cost_usd: f64 = records
                                .iter()
                                .filter(|r| r.timestamp > qts + 30 && r.timestamp < reset_ts)
                                .map(compute_claude_record_cost_usd)
                                .sum();
                            if post_cost_usd >= 0.05 {
                                let delta_pct = (post_cost_usd / budget.max(1.0)) * 100.0;
                                used = (used + delta_pct).clamp(0.0, 100.0);
                                source = "official+live".to_string();
                            }
                        }
                    }
                }

                return QuotaWindowStatus {
                    used_pct: used,
                    remaining_pct: (100.0 - used).clamp(0.0, 100.0),
                    used_tokens: local_fallback.used_tokens,
                    limit_tokens: None,
                    resets_at: Some(reset_ts),
                    source,
                };
            } else {
                // Official window already passed reset_ts!
                let post_reset_tokens: u64 = records
                    .iter()
                    .filter(|r| r.timestamp >= reset_ts)
                    .map(|r| quota_tokens_for_record(r, mode))
                    .sum();
                if post_reset_tokens == 0 {
                    // For Claude weekly, advance reset_ts by +7d multiples so the next Friday 04:00 reset is shown!
                    let next_reset = if provider_id == "claude" && local_fallback.resets_at.is_some() {
                        local_fallback.resets_at
                    } else {
                        None
                    };
                    return QuotaWindowStatus {
                        used_pct: 0.0,
                        remaining_pct: 100.0,
                        used_tokens: 0,
                        limit_tokens: None,
                        resets_at: next_reset,
                        source: "official".to_string(),
                    };
                }
            }
        } else {
            // Official entry has `resets_at: None` (e.g. Claude 5h = 0% idle before a session starts)
            let updated = snap_quota_updated_at.unwrap_or(0);
            let newer_local_tokens: u64 = if updated > 0 {
                records
                    .iter()
                    .filter(|r| r.timestamp > updated + 30)
                    .map(|r| quota_tokens_for_record(r, mode))
                    .sum()
            } else {
                0
            };

            if newer_local_tokens == 0 {
                let used = off.used_percentage.clamp(0.0, 999.0);
                return QuotaWindowStatus {
                    used_pct: used,
                    remaining_pct: (100.0 - used).clamp(0.0, 100.0),
                    used_tokens: 0,
                    limit_tokens: None,
                    resets_at: None,
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
            let d = dt.date_naive();
            if let Some(&idx) = date_to_idx.get(&d) {
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
            mk_rec(t0, 50_000, 50_000, 300_000),
            mk_rec(t0 + 3600, 30_000, 20_000, 400_000),
        ];

        let (status, _) = compute_session_block(
            &records,
            t0 + 7200,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        assert!((status.used_pct - 30.0).abs() < 0.01);
        assert!((status.remaining_pct - 70.0).abs() < 0.01);
        assert_eq!(status.used_tokens, 150_000);
        assert_eq!(status.resets_at, Some(t0 + FIVE_HOURS_SECS));

        let (expired, _) = compute_session_block(
            &records,
            t0 + FIVE_HOURS_SECS + 10,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        assert_eq!(expired.used_pct, 0.0);
        assert_eq!(expired.remaining_pct, 100.0);
        assert_eq!(expired.resets_at, None);
    }

    #[test]
    fn test_official_zero_without_resets_at_overrides_local_block() {
        let t0 = 1_700_000_000;
        let records = vec![mk_rec(t0, 100_000, 50_000, 0)];
        let (local, _) = compute_session_block(
            &records,
            t0 + 600,
            FIVE_HOURS_SECS,
            48 * 3600,
            500_000,
            AgyQuotaMode::ActiveOnly,
        );
        let official = RateLimitEntry {
            used_percentage: 0.0,
            resets_at: None,
        };
        let resolved = resolve_hybrid_window(
            "claude",
            Some(&official),
            Some(t0 + 590),
            &local,
            &records,
            AgyQuotaMode::ActiveOnly,
            Some(38.50),
            t0 + 600,
        );
        assert_eq!(resolved.used_pct, 0.0);
        assert_eq!(resolved.remaining_pct, 100.0);
        assert_eq!(resolved.source, "official");
    }

    #[test]
    fn test_claude_floored_5h_and_anchored_weekly_cost_math() {
        // 21:01:31 local -> 21:00:00 floored -> resets at 02:00:00 (+5h)
        let msg_ts = 1_794_146_491; // 2026-11-08 21:01:31 +07:00
        let floored = (msg_ts / 3600) * 3600;
        let rec = UsageRecord {
            timestamp: msg_ts,
            model: "Claude Opus 5.5".to_string(),
            input_tokens: 10,
            output_tokens: 896,
            cache_read_tokens: 713_059,
            cache_write_tokens: 124_464,
        };
        let (five_h, _) = compute_claude_5h_cost_window(&[rec], msg_ts + 300, 38.50);
        assert_eq!(five_h.resets_at, Some(floored + FIVE_HOURS_SECS));
        assert_eq!(five_h.remaining_pct.round() as i64, 97);

        // Weekly anchor Fri Nov 06 04:00 +07:00 (1793912400) -> next Fri Nov 13 04:00 +07:00 (1794517200)
        let anchor = 1_793_912_400;
        let weekly = compute_claude_weekly_cost_window(&[], msg_ts + 300, 437.50, Some(anchor));
        assert_eq!(weekly.resets_at, Some(anchor + SEVEN_DAYS_SECS));
    }
}
