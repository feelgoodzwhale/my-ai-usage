#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--dump-json") {
        let mut engine = my_usage_lib::EngineInner::default();
        let payload = engine.compute_snapshot();
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
        return;
    }

    my_usage_lib::run()
}
