use crate::config::QuotaDisplayMode;
use crate::quota::{FullUsagePayload, ProviderUsageReport};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Position, Rect, Size, WebviewWindow,
};

pub const CLAUDE_TRAY_ID: &str = "claude-tray";
pub const AGY_TRAY_ID: &str = "agy-tray";

#[derive(Default)]
pub struct TrayUiState {
    pub active_main_provider: Mutex<Option<String>>,
    pub hover_seq: AtomicU64,
    pub is_hovering_tray: AtomicBool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AlertTone {
    Normal,
    Warning,
    Critical,
}

fn tone_for_pct(pct: f64) -> AlertTone {
    if pct >= 90.0 {
        AlertTone::Critical
    } else if pct >= 75.0 {
        AlertTone::Warning
    } else {
        AlertTone::Normal
    }
}

fn rgb_for_tone(tone: AlertTone) -> (u8, u8, u8, bool) {
    match tone {
        AlertTone::Normal => (0, 0, 0, true),
        AlertTone::Warning => (245, 158, 11, false),
        AlertTone::Critical => (239, 68, 68, false),
    }
}

fn format_pct_compact(val: f64) -> String {
    let clamped = val.clamp(0.0, 100.0);
    if clamped >= 99.95 {
        "100%".to_string()
    } else if clamped <= 0.05 {
        "0%".to_string()
    } else if (clamped - clamped.round()).abs() >= 0.05 {
        format!("{:.1}%", clamped)
    } else {
        format!("{:.0}%", clamped)
    }
}

fn format_tray_title(report: &ProviderUsageReport, mode: QuotaDisplayMode) -> String {
    match mode {
        QuotaDisplayMode::Remaining => format_pct_compact(report.five_hour.remaining_pct),
        QuotaDisplayMode::Used => format_pct_compact(report.five_hour.used_pct),
    }
}

fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = px - ax;
    let wy = py - ay;
    let c1 = vx * wx + vy * wy;
    if c1 <= 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let c2 = vx * vx + vy * vy;
    if c2 <= c1 {
        return ((px - bx).powi(2) + (py - by).powi(2)).sqrt();
    }
    let b = c1 / c2;
    let pbx = ax + b * vx;
    let pby = ay + b * vy;
    ((px - pbx).powi(2) + (py - pby).powi(2)).sqrt()
}

fn smooth_coverage(dist: f32, radius: f32) -> f32 {
    ((radius + 0.65 - dist) / 1.3).clamp(0.0, 1.0)
}

fn render_claude_icon(r: u8, g: u8, b: u8) -> Image<'static> {
    let size: usize = 44;
    let mut rgba = vec![0u8; size * size * 4];
    let cx = 22.0f32;
    let cy = 22.0f32;

    let ray_count = 10;
    let mut segments = Vec::with_capacity(ray_count);
    for i in 0..ray_count {
        let angle = (i as f32) * std::f32::consts::TAU / (ray_count as f32) - 0.25;
        let inner = 3.2f32;
        let outer = if i % 2 == 0 { 16.5f32 } else { 13.5f32 };
        segments.push((
            cx + inner * angle.cos(),
            cy + inner * angle.sin(),
            cx + outer * angle.cos(),
            cy + outer * angle.sin(),
        ));
    }

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let mut min_d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - 2.6;
            for &(ax, ay, bx, by) in &segments {
                let d = dist_to_segment(px, py, ax, ay, bx, by) - 1.95;
                if d < min_d {
                    min_d = d;
                }
            }

            let alpha = (smooth_coverage(min_d, 0.0) * 255.0).round() as u8;
            let idx = (y * size + x) * 4;
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }
    }

    Image::new_owned(rgba, size as u32, size as u32)
}

fn render_agy_icon(r: u8, g: u8, b: u8) -> Image<'static> {
    let size: usize = 44;
    let mut rgba = vec![0u8; size * size * 4];

    let segments = [
        (22.0f32, 6.5f32, 8.0f32, 34.5f32),
        (22.0f32, 6.5f32, 36.0f32, 34.5f32),
        (8.0f32, 34.5f32, 16.5f32, 29.0f32),
        (36.0f32, 34.5f32, 27.5f32, 29.0f32),
    ];

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let mut min_d = f32::MAX;
            for &(ax, ay, bx, by) in &segments {
                let d = dist_to_segment(px, py, ax, ay, bx, by) - 2.25;
                if d < min_d {
                    min_d = d;
                }
            }
            let core_d = ((px - 22.0).powi(2) + (py - 24.5).powi(2)).sqrt() - 3.3;
            if core_d < min_d {
                min_d = core_d;
            }

            let alpha = (smooth_coverage(min_d, 0.0) * 255.0).round() as u8;
            let idx = (y * size + x) * 4;
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }
    }

    Image::new_owned(rgba, size as u32, size as u32)
}

pub fn init_trays(app: &AppHandle, tray_state: Arc<TrayUiState>) -> tauri::Result<()> {
    let claude_icon = render_claude_icon(0, 0, 0);
    let agy_icon = render_agy_icon(0, 0, 0);

    let st_claude = tray_state.clone();
    TrayIconBuilder::with_id(CLAUDE_TRAY_ID)
        .icon(claude_icon)
        .icon_as_template(true)
        .title("100%")
        .on_tray_icon_event(move |tray, event| {
            handle_tray_event(tray.app_handle(), "claude", event, &st_claude);
        })
        .build(app)?;

    let st_agy = tray_state;
    TrayIconBuilder::with_id(AGY_TRAY_ID)
        .icon(agy_icon)
        .icon_as_template(true)
        .title("100%")
        .on_tray_icon_event(move |tray, event| {
            handle_tray_event(tray.app_handle(), "agy", event, &st_agy);
        })
        .build(app)?;

    Ok(())
}

pub fn update_trays(app: &AppHandle, payload: &FullUsagePayload) {
    let mode = payload.config.quota_display_mode;

    if let Some(tray) = app.tray_by_id(CLAUDE_TRAY_ID) {
        let tone = tone_for_pct(payload.claude.five_hour.used_pct);
        let (r, g, b, is_template) = rgb_for_tone(tone);
        let _ = tray.set_icon(Some(render_claude_icon(r, g, b)));
        let _ = tray.set_icon_as_template(is_template);
        let _ = tray.set_title(Some(format_tray_title(&payload.claude, mode)));
    }

    if let Some(tray) = app.tray_by_id(AGY_TRAY_ID) {
        let tone = tone_for_pct(payload.agy.five_hour.used_pct);
        let (r, g, b, is_template) = rgb_for_tone(tone);
        let _ = tray.set_icon(Some(render_agy_icon(r, g, b)));
        let _ = tray.set_icon_as_template(is_template);
        let _ = tray.set_title(Some(format_tray_title(&payload.agy, mode)));
    }
}

fn handle_tray_event(
    app: &AppHandle,
    provider: &str,
    event: TrayIconEvent,
    state: &Arc<TrayUiState>,
) {
    match event {
        TrayIconEvent::Enter { rect, .. } => {
            state.is_hovering_tray.store(true, Ordering::SeqCst);
            state.hover_seq.fetch_add(1, Ordering::SeqCst);

            if let Some(main_win) = app.get_webview_window("main-popover") {
                if main_win.is_visible().unwrap_or(false) {
                    return;
                }
            }

            let _ = app.emit("hover-provider", provider);
            if let Some(hover_win) = app.get_webview_window("hover-card") {
                position_window_below_tray(&hover_win, &rect, 292.0);
                let _ = hover_win.show();
            }
        }
        TrayIconEvent::Leave { .. } => {
            state.is_hovering_tray.store(false, Ordering::SeqCst);
            let seq = state.hover_seq.fetch_add(1, Ordering::SeqCst) + 1;
            let app_clone = app.clone();
            let state_clone = state.clone();

            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(140));
                if !state_clone.is_hovering_tray.load(Ordering::SeqCst)
                    && state_clone.hover_seq.load(Ordering::SeqCst) == seq
                {
                    if let Some(hover_win) = app_clone.get_webview_window("hover-card") {
                        let _ = hover_win.hide();
                    }
                }
            });
        }
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            rect,
            ..
        } => {
            state.is_hovering_tray.store(false, Ordering::SeqCst);
            if let Some(hover_win) = app.get_webview_window("hover-card") {
                let _ = hover_win.hide();
            }

            let Some(main_win) = app.get_webview_window("main-popover") else {
                return;
            };

            let is_vis = main_win.is_visible().unwrap_or(false);
            let mut active_guard = state
                .active_main_provider
                .lock()
                .unwrap_or_else(|e| e.into_inner());

            if is_vis && active_guard.as_deref() == Some(provider) {
                *active_guard = None;
                let _ = main_win.hide();
                return;
            }

            *active_guard = Some(provider.to_string());
            drop(active_guard);

            let _ = app.emit("select-provider", provider);
            position_window_below_tray(&main_win, &rect, 396.0);
            let _ = main_win.show();
            let _ = main_win.set_focus();
        }
        _ => {}
    }
}

fn position_window_below_tray(win: &WebviewWindow, rect: &Rect, win_logical_width: f64) {
    let scale = win.scale_factor().unwrap_or(2.0);

    let (rx, ry) = match rect.position {
        Position::Logical(p) => (p.x, p.y),
        Position::Physical(p) => (p.x as f64 / scale, p.y as f64 / scale),
    };
    let (rw, rh) = match rect.size {
        Size::Logical(s) => (s.width, s.height),
        Size::Physical(s) => (s.width as f64 / scale, s.height as f64 / scale),
    };

    let mut target_x = rx + (rw / 2.0) - (win_logical_width / 2.0);
    let target_y = (ry + rh + 5.0).max(28.0);

    if let Ok(Some(mon)) = win.current_monitor() {
        let m_scale = mon.scale_factor();
        let mon_x = mon.position().x as f64 / m_scale;
        let mon_w = mon.size().width as f64 / m_scale;
        let min_x = mon_x + 8.0;
        let max_x = (mon_x + mon_w - win_logical_width - 8.0).max(min_x);
        target_x = target_x.clamp(min_x, max_x);
    }

    let _ = win.set_position(LogicalPosition::new(target_x, target_y));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::quota::QuotaWindowStatus;

    #[test]
    fn test_tray_title_defaults_to_remaining_and_only_shows_5h() {
        assert_eq!(
            AppConfig::default().quota_display_mode,
            QuotaDisplayMode::Remaining
        );

        let claude = ProviderUsageReport {
            five_hour: QuotaWindowStatus {
                used_pct: 0.0,
                remaining_pct: 100.0,
                ..Default::default()
            },
            weekly: QuotaWindowStatus {
                used_pct: 100.0,
                remaining_pct: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(format_tray_title(&claude, QuotaDisplayMode::Remaining), "100%");
        assert_eq!(format_tray_title(&claude, QuotaDisplayMode::Used), "0%");

        let agy = ProviderUsageReport {
            five_hour: QuotaWindowStatus {
                used_pct: 1.60,
                remaining_pct: 98.40,
                ..Default::default()
            },
            weekly: QuotaWindowStatus {
                used_pct: 22.02,
                remaining_pct: 77.98,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(format_tray_title(&agy, QuotaDisplayMode::Remaining), "98.4%");
        assert_eq!(format_tray_title(&agy, QuotaDisplayMode::Used), "1.6%");
    }
}
