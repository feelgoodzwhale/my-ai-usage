use crate::collector::{
    is_recently_modified, normalize_model_label, parse_rfc3339_epoch, read_incremental_lines,
    CachedFileState, UsageRecord,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Default)]
pub struct AgyCollector {
    files: HashMap<PathBuf, CachedFileState>,
}

fn extract_conversation_id(path: &Path) -> Option<String> {
    // .../brain/<conv-id>/.system_generated/logs/transcript.jsonl
    path.parent()?
        .parent()?
        .parent()?
        .file_name()?
        .to_str()
        .map(|s| s.to_string())
}

impl AgyCollector {
    pub fn collect(&mut self, fallback_model: Option<&str>) -> Vec<UsageRecord> {
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };
        let roots = [
            home.join(".gemini").join("antigravity").join("brain"),
            home.join(".gemini").join("antigravity-cli").join("brain"),
        ];

        let default_model = fallback_model
            .filter(|s| !s.trim().is_empty())
            .map(normalize_model_label)
            .or_else(read_agy_cli_default_model)
            .unwrap_or_else(|| "Gemini (AGY)".to_string());

        for root in &roots {
            if !root.exists() {
                continue;
            }
            for entry in WalkDir::new(root)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if path.file_name().and_then(|n| n.to_str()) != Some("transcript.jsonl") {
                    continue;
                }
                let path_buf = path.to_path_buf();
                if !self.files.contains_key(&path_buf) && !is_recently_modified(path, 35) {
                    continue;
                }

                let state = self.files.entry(path_buf.clone()).or_default();
                read_incremental_lines(&path_buf, state, |line_offset, line, cur_model, records| {
                    parse_agy_line(line_offset, line, cur_model, &default_model, records);
                });
            }
        }

        // Deduplicate across Desktop/IDE and CLI brain roots by (conversation_id, step_key)
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<UsageRecord> = Vec::new();
        for (path, state) in &self.files {
            let conv_prefix = extract_conversation_id(path)
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            for (step_key, rec) in &state.records {
                let global_key = format!("{conv_prefix}:{step_key}");
                if seen.insert(global_key) {
                    out.push(rec.clone());
                }
            }
        }

        out.sort_by_key(|r| r.timestamp);
        out
    }
}

fn read_agy_cli_default_model() -> Option<String> {
    let home = dirs::home_dir()?;
    let path = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("settings.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let model = v.get("model")?.as_str()?;
    Some(normalize_model_label(model))
}

fn extract_model_selection_change(content: &str) -> Option<String> {
    let block = if let Some(s) = content.find("<USER_SETTINGS_CHANGE>") {
        let rest_block = &content[s..];
        let e = rest_block
            .find("</USER_SETTINGS_CHANGE>")
            .unwrap_or(rest_block.len());
        &rest_block[..e]
    } else {
        return None;
    };

    let marker = "changed setting `Model Selection`";
    let idx = block.find(marker)?;
    let line_end = block[idx..].find('\n').unwrap_or(block.len() - idx);
    let same_line = &block[idx..idx + line_end];
    let to_idx = same_line.find(" to ")?;
    let rest = &same_line[to_idx + 4..];
    let end = rest.find(". ").unwrap_or(rest.len());
    let candidate = rest[..end].trim().trim_end_matches('.');
    if candidate.is_empty() || candidate.len() > 64 || candidate.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(normalize_model_label(candidate))
    }
}

pub fn parse_agy_line(
    line_offset: u64,
    line: &str,
    cur_model: &mut Option<String>,
    default_model: &str,
    records: &mut HashMap<String, UsageRecord>,
) {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return;
    };

    if let Some(content) = v.get("content").and_then(|c| c.as_str()) {
        if content.contains("Model Selection") {
            if let Some(new_model) = extract_model_selection_change(content) {
                *cur_model = Some(new_model);
            }
        }
    }

    if let Some(m) = v
        .get("model")
        .and_then(|m| m.as_str())
        .filter(|s| !s.is_empty())
    {
        *cur_model = Some(normalize_model_label(m));
    }

    let input_tokens = v.get("input_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
    let output_tokens = v.get("output_tokens").and_then(|n| n.as_u64()).unwrap_or(0);
    let cache_read_tokens = v
        .get("cache_read_tokens")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    if input_tokens == 0 && output_tokens == 0 && cache_read_tokens == 0 {
        return;
    }

    let Some(ts_str) = v.get("created_at").and_then(|t| t.as_str()) else {
        return;
    };
    let Some(timestamp) = parse_rfc3339_epoch(ts_str) else {
        return;
    };

    let model = cur_model
        .clone()
        .unwrap_or_else(|| default_model.to_string());

    let step_key = v
        .get("step_index")
        .and_then(|s| s.as_u64())
        .map(|idx| format!("step:{idx}"))
        .unwrap_or_else(|| format!("offset:{line_offset}"));

    records.insert(
        step_key,
        UsageRecord {
            timestamp,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens: 0,
        },
    );
}
