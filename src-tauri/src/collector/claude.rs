use crate::collector::{
    is_recently_modified, normalize_model_label, parse_rfc3339_epoch, read_incremental_lines,
    CachedFileState, UsageRecord,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Default)]
pub struct ClaudeCollector {
    files: HashMap<PathBuf, CachedFileState>,
}

impl ClaudeCollector {
    pub fn collect(&mut self) -> Vec<UsageRecord> {
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };
        let projects_dir = home.join(".claude").join("projects");
        if !projects_dir.exists() {
            return Vec::new();
        }

        for entry in WalkDir::new(&projects_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let path_buf = path.to_path_buf();
            if !self.files.contains_key(&path_buf) && !is_recently_modified(path, 35) {
                continue;
            }

            let state = self.files.entry(path_buf.clone()).or_default();
            read_incremental_lines(&path_buf, state, |line_offset, line, _cur_model, records| {
                if !line.contains("\"usage\"") {
                    return;
                }
                if let Some((dedup_key, rec)) = parse_claude_line(line_offset, line) {
                    match records.get(&dedup_key) {
                        Some(existing) if existing.total_tokens() > rec.total_tokens() => {}
                        _ => {
                            records.insert(dedup_key, rec);
                        }
                    }
                }
            });
        }

        let mut out: Vec<UsageRecord> = self
            .files
            .values()
            .flat_map(|s| s.records.values().cloned())
            .collect();
        out.sort_by_key(|r| r.timestamp);
        out
    }
}

pub fn parse_claude_line(line_offset: u64, line: &str) -> Option<(String, UsageRecord)> {
    let v: Value = serde_json::from_str(line).ok()?;
    let usage = v
        .get("message")
        .and_then(|m| m.get("usage"))
        .or_else(|| v.get("usage"))?;

    let ts_str = v.get("timestamp")?.as_str()?;
    let timestamp = parse_rfc3339_epoch(ts_str)?;

    let input_tokens = usage.get("input_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
    let output_tokens = usage.get("output_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let cache_write_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    if input_tokens == 0 && output_tokens == 0 && cache_read_tokens == 0 && cache_write_tokens == 0
    {
        return None;
    }

    let raw_model = v
        .get("requestedModel")
        .and_then(|m| m.as_str())
        .or_else(|| {
            v.get("message")
                .and_then(|m| m.get("model"))
                .and_then(|m| m.as_str())
        })
        .unwrap_or("Claude");

    if raw_model == "<synthetic>" {
        return None;
    }

    let model = normalize_model_label(raw_model);

    let msg_id = v
        .get("message")
        .and_then(|m| m.get("id"))
        .and_then(|s| s.as_str());
    let req_id = v.get("requestId").and_then(|s| s.as_str());
    let uuid = v.get("uuid").and_then(|s| s.as_str());

    let dedup_key = match (msg_id, req_id) {
        (Some(m), Some(r)) => format!("{m}:{r}"),
        (Some(m), None) => m.to_string(),
        (None, Some(r)) => r.to_string(),
        (None, None) => uuid
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("offset:{line_offset}")),
    };

    Some((
        dedup_key,
        UsageRecord {
            timestamp,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
        },
    ))
}
