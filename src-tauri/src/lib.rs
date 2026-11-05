pub mod collector;
pub mod config;
pub mod notifier;
pub mod quota;
pub mod tray;

use chrono::Utc;
use collector::agy::AgyCollector;
use collector::claude::ClaudeCollector;
use collector::statusline::{agy_status_path, claude_status_path, read_status_snapshot};
use config::{app_data_dir, AgyQuotaMode, AppConfig};
use notifier::UsageNotifier;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use quota::{build_provider_report, FullUsagePayload};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tray::{init_trays, update_trays, TrayUiState};

pub struct EngineInner {
    pub claude_collector: ClaudeCollector,
    pub agy_collector: AgyCollector,
    pub notifier: UsageNotifier,
    pub config: AppConfig,
    pub latest: Option<FullUsagePayload>,
}

impl Default for EngineInner {
    fn default() -> Self {
        Self {
            claude_collector: ClaudeCollector::default(),
            agy_collector: AgyCollector::default(),
            notifier: UsageNotifier::default(),
            config: AppConfig::load(),
            latest: None,
        }
    }
}

impl EngineInner {
    pub fn compute_snapshot(&mut self) -> FullUsagePayload {
        let now = Utc::now().timestamp();
        let claude_snap = read_status_snapshot(&claude_status_path());
        let agy_snap = read_status_snapshot(&agy_status_path());

        let claude_records = self.claude_collector.collect();
        let agy_records = self
            .agy_collector
            .collect(agy_snap.as_ref().and_then(|s| s.model_name.as_deref()));

        let claude_report = build_provider_report(
            "claude",
            "Claude",
            &claude_records,
            claude_snap.as_ref(),
            self.config.claude_fallback_five_hour_limit,
            self.config.claude_fallback_weekly_limit,
            AgyQuotaMode::ActiveOnly,
            now,
        );

        let agy_report = build_provider_report(
            "agy",
            "AGY",
            &agy_records,
            agy_snap.as_ref(),
            self.config.agy_five_hour_limit,
            self.config.agy_weekly_limit,
            self.config.agy_quota_mode,
            now,
        );

        let payload = FullUsagePayload {
            claude: claude_report,
            agy: agy_report,
            config: self.config.clone(),
            generated_at: now,
        };
        self.latest = Some(payload.clone());
        payload
    }
}

pub type SharedEngine = Arc<Mutex<EngineInner>>;

pub fn refresh_and_broadcast(app: &AppHandle, engine: &SharedEngine) -> FullUsagePayload {
    let payload = {
        let mut guard = engine.lock().unwrap_or_else(|e| e.into_inner());
        let payload = guard.compute_snapshot();
        let now = payload.generated_at;
        let cfg = guard.config.clone();
        guard
            .notifier
            .check_and_notify(app, &payload.claude, &cfg, now);
        guard
            .notifier
            .check_and_notify(app, &payload.agy, &cfg, now);
        payload
    };

    update_trays(app, &payload);
    let _ = app.emit("usage-updated", &payload);
    payload
}

#[tauri::command]
fn get_usage_data(app: AppHandle, engine: State<'_, SharedEngine>) -> FullUsagePayload {
    {
        let guard = engine.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref cached) = guard.latest {
            return cached.clone();
        }
    }
    refresh_and_broadcast(&app, engine.inner())
}

#[tauri::command]
fn force_refresh(app: AppHandle, engine: State<'_, SharedEngine>) -> FullUsagePayload {
    refresh_and_broadcast(&app, engine.inner())
}

#[tauri::command]
fn save_app_config(
    app: AppHandle,
    engine: State<'_, SharedEngine>,
    config: AppConfig,
) -> Result<FullUsagePayload, String> {
    config.save()?;
    {
        let mut guard = engine.lock().unwrap_or_else(|e| e.into_inner());
        guard.config = config;
    }
    Ok(refresh_and_broadcast(&app, engine.inner()))
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn setup_vibrancy(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        for label in ["main-popover", "hover-card"] {
            if let Some(win) = app.get_webview_window(label) {
                let _ = apply_vibrancy(
                    &win,
                    NSVisualEffectMaterial::Popover,
                    Some(NSVisualEffectState::Active),
                    Some(14.0),
                );
            }
        }
    }
}

fn spawn_background_watchers(app: AppHandle, engine: SharedEngine) {
    let _ = std::fs::create_dir_all(app_data_dir());

    // 1. File watcher thread with 400ms debounce
    let app_fs = app.clone();
    let engine_fs = engine.clone();
    std::thread::spawn(move || {
        let (tx, rx) = channel();
        let Ok(mut watcher) = RecommendedWatcher::new(tx, NotifyConfig::default()) else {
            return;
        };

        let _ = watcher.watch(&app_data_dir(), RecursiveMode::NonRecursive);
        if let Some(home) = dirs::home_dir() {
            let paths = [
                home.join(".claude").join("projects"),
                home.join(".gemini").join("antigravity").join("brain"),
                home.join(".gemini").join("antigravity-cli").join("brain"),
            ];
            for p in &paths {
                if p.exists() {
                    let _ = watcher.watch(p, RecursiveMode::Recursive);
                }
            }
        }

        let mut last_refresh = Instant::now() - Duration::from_secs(5);
        while let Ok(res) = rx.recv() {
            if res.is_ok() {
                if last_refresh.elapsed() >= Duration::from_millis(400) {
                    // Drain queued burst events after short settle window
                    std::thread::sleep(Duration::from_millis(250));
                    while rx.try_recv().is_ok() {}
                    let _ = refresh_and_broadcast(&app_fs, &engine_fs);
                    last_refresh = Instant::now();
                }
            }
        }
    });

    // 2. Periodic 30s ticker for reset countdown rollovers
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let _ = refresh_and_broadcast(&app, &engine);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine: SharedEngine = Arc::new(Mutex::new(EngineInner::default()));
    let tray_ui_state = Arc::new(TrayUiState::default());

    let engine_setup = engine.clone();
    let tray_state_setup = tray_ui_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(engine)
        .manage(tray_ui_state.clone())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            setup_vibrancy(&handle);
            init_trays(&handle, tray_state_setup)?;

            // Auto-hide main-popover when focus leaves it
            if let Some(main_win) = handle.get_webview_window("main-popover") {
                let win_clone = main_win.clone();
                let st = tray_ui_state.clone();
                main_win.on_window_event(move |ev| {
                    if let WindowEvent::Focused(false) = ev {
                        if let Ok(mut g) = st.active_main_provider.lock() {
                            *g = None;
                        }
                        let _ = win_clone.hide();
                    }
                });
            }

            // Initial immediate compute + tray update
            let _ = refresh_and_broadcast(&handle, &engine_setup);
            spawn_background_watchers(handle, engine_setup);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_usage_data,
            force_refresh,
            save_app_config,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running my-ai-usage tauri application");
}
