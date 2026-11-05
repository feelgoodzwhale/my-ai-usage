use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyQuotaMode {
    ActiveOnly,
    WeightedCache,
    IncludeAllCache,
}

impl Default for AgyQuotaMode {
    fn default() -> Self {
        Self::ActiveOnly
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub agy_five_hour_limit: u64,
    pub agy_weekly_limit: u64,
    pub claude_fallback_five_hour_limit: u64,
    pub claude_fallback_weekly_limit: u64,
    pub agy_quota_mode: AgyQuotaMode,
    pub notify_enabled: bool,
    pub notify_warn_pct: f64,
    pub notify_crit_pct: f64,
    pub notify_on_reset: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            agy_five_hour_limit: 5_000_000,
            agy_weekly_limit: 25_000_000,
            claude_fallback_five_hour_limit: 500_000,
            claude_fallback_weekly_limit: 5_000_000,
            agy_quota_mode: AgyQuotaMode::ActiveOnly,
            notify_enabled: true,
            notify_warn_pct: 80.0,
            notify_crit_pct: 95.0,
            notify_on_reset: true,
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(".local").join("share").join("my-usage")
}

pub fn config_file_path() -> PathBuf {
    app_data_dir().join("config.json")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_file_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&raw) {
                return cfg;
            }
        }
        let cfg = Self::default();
        let _ = cfg.save();
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = app_data_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(config_file_path(), json).map_err(|e| e.to_string())
    }
}
