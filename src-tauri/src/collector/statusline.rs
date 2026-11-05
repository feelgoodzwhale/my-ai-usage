use crate::collector::parse_rfc3339_epoch;
use crate::config::app_data_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimitEntry {
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubQuotaBucket {
    pub key: String,
    pub label: String,
    pub window_kind: String, // "5h" | "weekly"
    pub used_pct: f64,
    pub remaining_pct: f64,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatuslineSnapshot {
    pub five_hour: Option<RateLimitEntry>,
    pub seven_day: Option<RateLimitEntry>,
    pub model_name: Option<String>,
    pub plan_tier: Option<String>,
    pub context_used_pct: Option<f64>,
    pub quota_buckets: Vec<SubQuotaBucket>,
    pub updated_at: Option<i64>,
}

pub fn claude_status_path() -> PathBuf {
    app_data_dir().join("claude-status.json")
}

pub fn claude_dotfile_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude.json"))
}

pub fn agy_status_path() -> PathBuf {
    app_data_dir().join("agy-status.json")
}

fn file_mtime_epoch(path: &PathBuf) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

fn parse_timestamp_value(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| {
            v.as_str().and_then(|s| {
                s.parse::<i64>()
                    .ok()
                    .or_else(|| parse_rfc3339_epoch(s))
            })
        })
}

fn parse_rate_limit_obj(val: Option<&Value>) -> Option<RateLimitEntry> {
    let obj = val?.as_object()?;
    let used_percentage = obj
        .get("used_percentage")
        .or_else(|| obj.get("utilization"))
        .and_then(|n| n.as_f64())?;
    let resets_at = obj.get("resets_at").and_then(parse_timestamp_value);
    Some(RateLimitEntry {
        used_percentage,
        resets_at,
    })
}

/// Reads Claude's official quota from both `~/.claude.json` (`cachedUsageUtilization`)
/// and `~/.local/share/my-ai-usage/claude-status.json`, merging whichever is freshest.
pub fn read_claude_snapshot() -> Option<StatuslineSnapshot> {
    let status_snap = read_claude_statusline_file(&claude_status_path());
    let dotfile_snap = claude_dotfile_path().and_then(|p| read_claude_dotfile_utilization(&p));

    match (status_snap, dotfile_snap) {
        (Some(st), Some(df)) => {
            let st_ts = st.updated_at.unwrap_or(0);
            let df_ts = df.updated_at.unwrap_or(0);

            let five_hour = merge_rate_entry(st.five_hour, st_ts, df.five_hour, df_ts);
            let seven_day = merge_rate_entry(st.seven_day, st_ts, df.seven_day, df_ts);
            let mut quota_buckets = df.quota_buckets;
            if quota_buckets.is_empty() {
                quota_buckets = st.quota_buckets;
            }

            Some(StatuslineSnapshot {
                five_hour,
                seven_day,
                model_name: st.model_name.or(df.model_name),
                plan_tier: st.plan_tier.or(df.plan_tier),
                context_used_pct: st.context_used_pct,
                quota_buckets,
                updated_at: Some(st_ts.max(df_ts)),
            })
        }
        (Some(st), None) => Some(st),
        (None, Some(df)) => Some(df),
        (None, None) => None,
    }
}

fn merge_rate_entry(
    a: Option<RateLimitEntry>,
    a_ts: i64,
    b: Option<RateLimitEntry>,
    b_ts: i64,
) -> Option<RateLimitEntry> {
    match (a, b) {
        (Some(ea), Some(eb)) => {
            let (mut primary, secondary) = if a_ts >= b_ts { (ea, eb) } else { (eb, ea) };
            if primary.resets_at.is_none() && secondary.resets_at.is_some() {
                primary.resets_at = secondary.resets_at;
            }
            Some(primary)
        }
        (Some(ea), None) => Some(ea),
        (None, Some(eb)) => Some(eb),
        (None, None) => None,
    }
}

fn read_claude_statusline_file(path: &PathBuf) -> Option<StatuslineSnapshot> {
    let raw = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;

    let rate_limits = v.get("rate_limits");
    let five_hour = parse_rate_limit_obj(rate_limits.and_then(|r| r.get("five_hour")));
    let seven_day = parse_rate_limit_obj(rate_limits.and_then(|r| r.get("seven_day")));

    let model_name = v
        .get("model")
        .and_then(|m| {
            m.get("display_name")
                .and_then(|s| s.as_str())
                .or_else(|| m.get("id").and_then(|s| s.as_str()))
                .or_else(|| m.as_str())
        })
        .map(|s| s.to_string());

    let context_used_pct = v
        .get("context_window")
        .and_then(|c| c.get("used_percentage"))
        .and_then(|p| p.as_f64());

    let updated_at = v
        .get("_captured_at")
        .and_then(|t| t.as_i64())
        .or_else(|| file_mtime_epoch(path));

    Some(StatuslineSnapshot {
        five_hour,
        seven_day,
        model_name,
        plan_tier: Some("Claude".to_string()),
        context_used_pct,
        quota_buckets: Vec::new(),
        updated_at,
    })
}

fn read_claude_dotfile_utilization(path: &PathBuf) -> Option<StatuslineSnapshot> {
    let raw = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let cu = v.get("cachedUsageUtilization")?;
    let util = cu.get("utilization")?;

    let five_hour = parse_rate_limit_obj(util.get("five_hour"));
    let seven_day = parse_rate_limit_obj(util.get("seven_day"));

    let updated_at = cu
        .get("fetchedAtMs")
        .and_then(|ms| ms.as_i64().map(|n| n / 1000))
        .or_else(|| file_mtime_epoch(path));

    let mut quota_buckets = Vec::new();
    if let Some(fh) = &five_hour {
        quota_buckets.push(SubQuotaBucket {
            key: "claude-5h".to_string(),
            label: "Session Limit (5h)".to_string(),
            window_kind: "5h".to_string(),
            used_pct: fh.used_percentage.clamp(0.0, 100.0),
            remaining_pct: (100.0 - fh.used_percentage).clamp(0.0, 100.0),
            resets_at: fh.resets_at,
        });
    }
    if let Some(sd) = &seven_day {
        quota_buckets.push(SubQuotaBucket {
            key: "claude-weekly".to_string(),
            label: "Weekly Limit (All Models)".to_string(),
            window_kind: "weekly".to_string(),
            used_pct: sd.used_percentage.clamp(0.0, 100.0),
            remaining_pct: (100.0 - sd.used_percentage).clamp(0.0, 100.0),
            resets_at: sd.resets_at,
        });
    }
    for (k, title) in [
        ("seven_day_opus", "Weekly Opus Limit"),
        ("seven_day_sonnet", "Weekly Sonnet Limit"),
    ] {
        if let Some(entry) = parse_rate_limit_obj(util.get(k)) {
            quota_buckets.push(SubQuotaBucket {
                key: k.to_string(),
                label: title.to_string(),
                window_kind: "weekly".to_string(),
                used_pct: entry.used_percentage.clamp(0.0, 100.0),
                remaining_pct: (100.0 - entry.used_percentage).clamp(0.0, 100.0),
                resets_at: entry.resets_at,
            });
        }
    }

    Some(StatuslineSnapshot {
        five_hour,
        seven_day,
        model_name: None,
        plan_tier: Some("Claude Pro / Max".to_string()),
        context_used_pct: None,
        quota_buckets,
        updated_at,
    })
}

fn agy_bucket_label_and_order(key: &str) -> (usize, String, &'static str) {
    match key {
        "gemini-5h" => (1, "Gemini Models (5h)".to_string(), "5h"),
        "gemini-weekly" => (2, "Gemini Models (Weekly)".to_string(), "weekly"),
        "gemini-argon-wtu-weekly" => (3, "Gemini Argon (Weekly)".to_string(), "weekly"),
        "3p-5h" => (4, "Claude & GPT Models (5h)".to_string(), "5h"),
        "3p-weekly" => (5, "Claude & GPT Models (Weekly)".to_string(), "weekly"),
        other => {
            let kind = if other.contains("5h") { "5h" } else { "weekly" };
            (99, other.to_string(), kind)
        }
    }
}

/// Reads AGY's official statusline JSON (`.quota` map with `remaining_fraction` and `reset_time`).
pub fn read_agy_snapshot() -> Option<StatuslineSnapshot> {
    let primary = agy_status_path();
    let legacy = dirs::home_dir()
        .map(|h| h.join(".local").join("share").join("my-usage").join("agy-status.json"));

    let path = if primary.exists() {
        primary
    } else if let Some(lp) = legacy.filter(|p| p.exists()) {
        lp
    } else {
        return None;
    };

    let raw = fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;

    let updated_at = v
        .get("_captured_at")
        .and_then(|t| t.as_i64())
        .or_else(|| file_mtime_epoch(&path));

    let model_name = v
        .get("model")
        .and_then(|m| {
            m.get("display_name")
                .and_then(|s| s.as_str())
                .or_else(|| m.get("id").and_then(|s| s.as_str()))
                .or_else(|| m.as_str())
        })
        .map(|s| s.to_string());

    let plan_tier = v
        .get("plan_tier")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let context_used_pct = v
        .get("context_window")
        .and_then(|c| c.get("used_percentage"))
        .and_then(|p| p.as_f64());

    let mut ordered_buckets: Vec<(usize, SubQuotaBucket)> = Vec::new();
    if let Some(quota_map) = v.get("quota").and_then(|q| q.as_object()) {
        for (k, val) in quota_map {
            let Some(rem_frac) = val.get("remaining_fraction").and_then(|f| f.as_f64()) else {
                continue;
            };
            let rem_clamped = rem_frac.clamp(0.0, 1.0);
            let remaining_pct = rem_clamped * 100.0;
            let used_pct = (1.0 - rem_clamped) * 100.0;

            let resets_at = val
                .get("reset_time")
                .and_then(parse_timestamp_value)
                .or_else(|| {
                    let secs = val.get("reset_in_seconds")?.as_i64()?;
                    Some(updated_at.unwrap_or(0) + secs)
                });

            let (ord, label, kind) = agy_bucket_label_and_order(k);
            ordered_buckets.push((
                ord,
                SubQuotaBucket {
                    key: k.clone(),
                    label,
                    window_kind: kind.to_string(),
                    used_pct,
                    remaining_pct,
                    resets_at,
                },
            ));
        }
    }

    ordered_buckets.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.key.cmp(&b.1.key)));
    let quota_buckets: Vec<SubQuotaBucket> = ordered_buckets.into_iter().map(|(_, b)| b).collect();

    let is_3p_active = model_name
        .as_deref()
        .map(|m| {
            let l = m.to_ascii_lowercase();
            l.contains("claude") || l.contains("gpt")
        })
        .unwrap_or(false);

    let target_5h_key = if is_3p_active { "3p-5h" } else { "gemini-5h" };
    let target_weekly_key = if is_3p_active {
        "3p-weekly"
    } else {
        "gemini-weekly"
    };

    let five_hour = quota_buckets
        .iter()
        .find(|b| b.key == target_5h_key)
        .or_else(|| quota_buckets.iter().find(|b| b.key == "gemini-5h"))
        .map(|b| RateLimitEntry {
            used_percentage: b.used_pct,
            resets_at: b.resets_at,
        })
        .or_else(|| parse_rate_limit_obj(v.get("rate_limits").and_then(|r| r.get("five_hour"))));

    let seven_day = quota_buckets
        .iter()
        .find(|b| b.key == target_weekly_key)
        .or_else(|| quota_buckets.iter().find(|b| b.key == "gemini-weekly"))
        .map(|b| RateLimitEntry {
            used_percentage: b.used_pct,
            resets_at: b.resets_at,
        })
        .or_else(|| parse_rate_limit_obj(v.get("rate_limits").and_then(|r| r.get("seven_day"))));

    Some(StatuslineSnapshot {
        five_hour,
        seven_day,
        model_name,
        plan_tier,
        context_used_pct,
        quota_buckets,
        updated_at,
    })
}
