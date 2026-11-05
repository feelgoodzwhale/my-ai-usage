use crate::config::AppConfig;
use crate::quota::ProviderUsageReport;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Default, Clone)]
struct ProviderAlertState {
    warned_level: u8, // 0 = none, 1 = warn, 2 = crit
    active_reset_ts: Option<i64>,
    had_significant_usage: bool,
}

#[derive(Default)]
pub struct UsageNotifier {
    states: HashMap<String, ProviderAlertState>,
}

impl UsageNotifier {
    pub fn check_and_notify(
        &mut self,
        app: &AppHandle,
        report: &ProviderUsageReport,
        cfg: &AppConfig,
        now: i64,
    ) {
        let state = self.states.entry(report.id.clone()).or_default();
        let pct = report.five_hour.used_pct;
        let cur_reset = report.five_hour.resets_at;

        // Detect window reset completion
        if let Some(prev_reset) = state.active_reset_ts {
            let window_rolled_over = match cur_reset {
                None => now >= prev_reset,
                Some(new_r) => new_r > prev_reset + 60,
            };
            if window_rolled_over {
                if cfg.notify_enabled && cfg.notify_on_reset && state.had_significant_usage {
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("{} — รีเซ็ตโควต้า 5 ชม. แล้ว", report.name))
                        .body(format!(
                            "รอบโควต้า 5 ชั่วโมงของ {} รีเซ็ตกลับเป็น 0% พร้อมใช้งานรอบใหม่แล้ว",
                            report.name
                        ))
                        .show();
                }
                state.warned_level = 0;
                state.had_significant_usage = false;
            }
        }

        state.active_reset_ts = cur_reset;
        if pct >= 15.0 {
            state.had_significant_usage = true;
        }

        if !cfg.notify_enabled {
            return;
        }

        if pct >= cfg.notify_crit_pct && state.warned_level < 2 {
            state.warned_level = 2;
            let _ = app
                .notification()
                .builder()
                .title(format!("⚠️ {} ใกล้เต็มโควต้า ({:.0}%)", report.name, pct))
                .body(format!(
                    "การใช้งานรอบ 5 ชั่วโมงแตะ {:.0}% แล้ว",
                    pct
                ))
                .show();
        } else if pct >= cfg.notify_warn_pct && state.warned_level < 1 {
            state.warned_level = 1;
            let _ = app
                .notification()
                .builder()
                .title(format!("{} ใช้โควต้าไป {:.0}%", report.name, pct))
                .body(format!(
                    "การใช้งานรอบ 5 ชั่วโมงเกินเกณฑ์เตือน {:.0}% แล้ว",
                    cfg.notify_warn_pct
                ))
                .show();
        } else if pct < cfg.notify_warn_pct * 0.5 {
            state.warned_level = 0;
        }
    }
}
