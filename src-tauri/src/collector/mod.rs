pub mod agy;
pub mod claude;
pub mod statusline;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: i64,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl UsageRecord {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            + self.output_tokens
            + self.cache_read_tokens
            + self.cache_write_tokens
    }
}

#[derive(Debug, Default)]
pub struct CachedFileState {
    pub offset: u64,
    pub last_len: u64,
    pub current_model: Option<String>,
    pub records: HashMap<String, UsageRecord>,
}

pub fn parse_rfc3339_epoch(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

pub fn is_recently_modified(path: &Path, max_age_days: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(max_age_days * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    modified >= cutoff
}

/// Reads only newly appended complete (`\n`-terminated) lines from `path` starting at `state.offset`.
/// If the file shrank (rewritten/truncated), resets state and reads from byte 0.
pub fn read_incremental_lines<F>(path: &PathBuf, state: &mut CachedFileState, mut on_line: F)
where
    F: FnMut(u64, &str, &mut Option<String>, &mut HashMap<String, UsageRecord>),
{
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    let file_len = meta.len();
    if file_len == state.last_len && state.offset > 0 {
        return;
    }
    if file_len < state.last_len {
        state.offset = 0;
        state.last_len = 0;
        state.current_model = None;
        state.records.clear();
    }

    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(state.offset)).is_err() {
        return;
    }

    let mut reader = BufReader::new(file);
    let mut current_offset = state.offset;
    let mut buf = String::new();

    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(bytes_read) => {
                // Only advance past lines terminated by '\n' so half-written lines are retried next tick.
                if !buf.ends_with('\n') && (current_offset + bytes_read as u64) == file_len {
                    break;
                }
                let line_offset = current_offset;
                current_offset += bytes_read as u64;
                let trimmed = buf.trim();
                if !trimmed.is_empty() {
                    on_line(
                        line_offset,
                        trimmed,
                        &mut state.current_model,
                        &mut state.records,
                    );
                }
            }
            Err(_) => break,
        }
    }

    state.offset = current_offset;
    state.last_len = current_offset;
}

pub fn normalize_model_label(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") || trimmed == "<synthetic>" {
        return "Unknown Model".to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("claude-") {
        // Strip trailing date suffix like -20250929 (8 digits at end)
        let mut parts: Vec<&str> = trimmed.split('-').collect();
        if let Some(last) = parts.last() {
            if last.len() == 8 && last.chars().all(|c| c.is_ascii_digit()) {
                parts.pop();
            }
        }
        // Format e.g. claude-opus-5-5 -> Claude Opus 5.5
        if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("claude") {
            let family = capitalize(parts[1]);
            let ver_parts = &parts[2..];
            if ver_parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
                return format!("Claude {} {}", family, ver_parts.join("."));
            }
            return format!("Claude {} {}", family, ver_parts.join(" "));
        }
    }

    if lower.starts_with("gemini-") {
        let parts: Vec<&str> = trimmed.split('-').collect();
        let formatted: Vec<String> = parts.into_iter().map(capitalize).collect();
        return formatted.join(" ");
    }

    trimmed.to_string()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
