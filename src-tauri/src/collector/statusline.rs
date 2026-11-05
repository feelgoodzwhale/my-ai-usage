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
pub struct StatuslineSnapshot {
    pub five_hour: Option<RateLimitEntry>,
    pub seven_day: Option<RateLimitEntry>,
    pub model_name: Option<String>,
    pub plan_tier: Option<String>,
    pub context_used_pct: Option<f64>,
    pub updated_at: Option<i64>,
}

pub fn claude_status_path() -> PathBuf {
    app_data_dir().join("claude-status.json")
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

fn parse_rate_limit(val: Option<&Value>) -> Option<RateLimitEntry> {
    let obj = val?.as_object()?;
    let used_percentage = obj.get("used_percentage")?.as_f64()?;
    let resets_at = obj.get("resets_at").and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_f64().map(|f| f as i64))
            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    });
    Some(RateLimitEntry {
        used_percentage,
        resets_at,
    })
}

pub fn read_status_snapshot(path: &PathBuf) -> Option<StatuslineSnapshot> {
    let raw = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;

    let rate_limits = v.get("rate_limits");
    let five_hour = parse_rate_limit(rate_limits.and_then(|r| r.get("five_hour")));
    let seven_day = parse_rate_limit(rate_limits.and_then(|r| r.get("seven_day")));

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

    let updated_at = v
        .get("_captured_at")
        .and_then(|t| t.as_i64())
        .or_else(|| file_mtime_epoch(path));

    Some(StatuslineSnapshot {
        five_hour,
        seven_day,
        model_name,
        plan_tier,
        context_used_pct,
        updated_at,
    })
}
