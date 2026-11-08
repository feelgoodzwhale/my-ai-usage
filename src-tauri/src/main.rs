#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--dump-json") {
        if args.iter().any(|a| a == "--probe") {
            let _ = my_ai_usage_lib::collector::live_probe::probe_live_quotas(true);
        }
        let mut engine = my_ai_usage_lib::EngineInner::default();
        let payload = engine.compute_snapshot();
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
        return;
    }

    my_ai_usage_lib::run()
}
