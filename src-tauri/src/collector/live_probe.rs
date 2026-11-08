use crate::collector::statusline::{agy_status_path, claude_dotfile_path, claude_status_path};
use chrono::Utc;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

static PROBE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static LAST_AGY_PROBE_AT: AtomicI64 = AtomicI64::new(0);
static LAST_CLAUDE_PROBE_AT: AtomicI64 = AtomicI64::new(0);

const MIN_AUTO_PROBE_INTERVAL_SECS: i64 = 120;
const STALE_QUOTA_THRESHOLD_SECS: i64 = 240;

fn resolve_bin(name: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("bin").join(name));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(name));
    candidates.push(PathBuf::from("/usr/local/bin").join(name));
    candidates.push(PathBuf::from("/usr/bin").join(name));

    candidates.into_iter().find(|p| p.is_file())
}

fn read_json_quota_ts(path: &PathBuf) -> i64 {
    let Ok(raw) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return 0;
    };
    if let Some(qts) = v.get("_quota_captured_at").and_then(|t| t.as_i64()) {
        return qts;
    }
    if v.get("quota").is_some_and(|q| !q.is_null())
        || v.get("rate_limits").is_some_and(|r| !r.is_null())
    {
        return v.get("_captured_at").and_then(|t| t.as_i64()).unwrap_or(0);
    }
    0
}

fn read_claude_dotfile_ts() -> i64 {
    let Some(path) = claude_dotfile_path() else {
        return 0;
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return 0;
    };
    v.get("cachedUsageUtilization")
        .and_then(|c| c.get("fetchedAtMs"))
        .and_then(|ms| ms.as_i64())
        .map(|ms| ms / 1000)
        .unwrap_or(0)
}

fn find_trusted_claude_cwd() -> PathBuf {
    if let Some(dotfile) = claude_dotfile_path() {
        if let Ok(raw) = fs::read_to_string(dotfile) {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                if let Some(projects) = v.get("projects").and_then(|p| p.as_object()) {
                    for (dir, meta) in projects {
                        let trusted = meta
                            .get("hasTrustDialogAccepted")
                            .and_then(|b| b.as_bool())
                            .unwrap_or(false);
                        let p = PathBuf::from(dir);
                        if trusted && p.is_dir() {
                            return p;
                        }
                    }
                }
            }
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn is_claude_cli_logged_in(claude_bin: &PathBuf) -> bool {
    let mut env_path = String::from("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    if let Some(home) = dirs::home_dir() {
        env_path = format!("{}/.local/bin:{}", home.display(), env_path);
    }
    let out = Command::new(claude_bin)
        .args(["auth", "status"])
        .env("PATH", env_path)
        .env_remove("CLAUDECODE")
        .output();

    let Ok(output) = out else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if let Ok(v) = serde_json::from_str::<Value>(&text[start..=end]) {
                return v
                    .get("loggedIn")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
            }
        }
    }
    false
}

fn run_pty_agy_refresh(python_bin: &PathBuf, agy_bin: &PathBuf) -> bool {
    let status_path = agy_status_path();
    let before_ts = read_json_quota_ts(&status_path);

    let py_script = r#"
import fcntl, json, os, pty, select, struct, subprocess, sys, termios, time

agy_bin, status_file, before_ts = sys.argv[1], sys.argv[2], int(sys.argv[3])

def get_qts():
    try:
        with open(status_file, "r") as f:
            v = json.load(f)
        if v.get("quota") is not None:
            return int(v.get("_quota_captured_at") or v.get("_captured_at") or 0)
    except Exception:
        pass
    return 0

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 120, 0, 0))
env = os.environ.copy()
env["TERM"] = "xterm-256color"
proc = subprocess.Popen(
    [agy_bin],
    stdin=slave, stdout=slave, stderr=slave,
    close_fds=True, env=env, cwd="/tmp"
)
os.close(slave)

deadline = time.time() + 5.0
updated = False
while time.time() < deadline:
    r, _, _ = select.select([master], [], [], 0.12)
    if master in r:
        try:
            if not os.read(master, 4096):
                break
        except OSError:
            break
    if get_qts() > before_ts:
        updated = True
        break
    if proc.poll() is not None:
        break

try:
    os.write(master, b"\x04\x04")
except OSError:
    pass

end_wait = time.time() + 0.5
while time.time() < end_wait and proc.poll() is None:
    r, _, _ = select.select([master], [], [], 0.1)
    if master in r:
        try:
            os.read(master, 4096)
        except OSError:
            break

if proc.poll() is None:
    proc.terminate()
    try:
        proc.wait(timeout=1.0)
    except Exception:
        proc.kill()
os.close(master)
sys.exit(0 if (updated or get_qts() > before_ts) else 1)
"#;

    let mut env_path = String::from("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    if let Some(home) = dirs::home_dir() {
        env_path = format!("{}/.local/bin:{}", home.display(), env_path);
    }

    Command::new(python_bin)
        .arg("-c")
        .arg(py_script)
        .arg(agy_bin)
        .arg(&status_path)
        .arg(before_ts.to_string())
        .env("PATH", env_path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_pty_claude_refresh(python_bin: &PathBuf, claude_bin: &PathBuf) -> bool {
    let status_path = claude_status_path();
    let dotfile_path = claude_dotfile_path().unwrap_or_else(|| PathBuf::from("/nonexistent"));
    let before_ts = read_json_quota_ts(&status_path).max(read_claude_dotfile_ts());
    let cwd = find_trusted_claude_cwd();

    let py_script = r#"
import fcntl, json, os, pty, select, struct, subprocess, sys, termios, time

claude_bin, status_file, dotfile, cwd_dir, before_ts = (
    sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
)

def get_qts():
    best = 0
    try:
        with open(status_file, "r") as f:
            v = json.load(f)
        if v.get("rate_limits") is not None:
            best = max(best, int(v.get("_quota_captured_at") or v.get("_captured_at") or 0))
    except Exception:
        pass
    try:
        with open(dotfile, "r") as f:
            v = json.load(f)
        ms = ((v.get("cachedUsageUtilization") or {}).get("fetchedAtMs") or 0)
        best = max(best, int(ms) // 1000)
    except Exception:
        pass
    return best

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
env = os.environ.copy()
env["TERM"] = "xterm-256color"
env.pop("CLAUDECODE", None)
proc = subprocess.Popen(
    [claude_bin],
    stdin=slave, stdout=slave, stderr=slave,
    close_fds=True, env=env, cwd=cwd_dir
)
os.close(slave)

def drain(secs):
    end = time.time() + secs
    while time.time() < end:
        r, _, _ = select.select([master], [], [], 0.12)
        if master in r:
            try:
                if not os.read(master, 4096):
                    break
            except OSError:
                break
        if get_qts() > before_ts or proc.poll() is not None:
            return

drain(2.5)
if get_qts() <= before_ts and proc.poll() is None:
    try:
        os.write(master, b"/usage\r")
    except OSError:
        pass
    drain(2.5)
    try:
        os.write(master, b"\x1b")
    except OSError:
        pass
    drain(0.3)

try:
    os.write(master, b"/exit\r")
except OSError:
    pass
drain(0.5)

if proc.poll() is None:
    proc.terminate()
    try:
        proc.wait(timeout=1.0)
    except Exception:
        proc.kill()
os.close(master)
sys.exit(0 if get_qts() > before_ts else 1)
"#;

    let mut env_path = String::from("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    if let Some(home) = dirs::home_dir() {
        env_path = format!("{}/.local/bin:{}", home.display(), env_path);
    }

    Command::new(python_bin)
        .arg("-c")
        .arg(py_script)
        .arg(claude_bin)
        .arg(&status_path)
        .arg(&dotfile_path)
        .arg(&cwd)
        .arg(before_ts.to_string())
        .env("PATH", env_path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Runs zero-token headless PTY probes against installed `agy` and `claude` CLIs
/// when official quota snapshots are stale or `force` is true.
pub fn probe_live_quotas(force: bool) -> bool {
    if PROBE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }

    let result = std::panic::catch_unwind(|| {
        let Some(python_bin) = resolve_bin("python3") else {
            return false;
        };
        let now = Utc::now().timestamp();
        let mut any_updated = false;

        // 1. AGY live quota refresh
        if let Some(agy_bin) = resolve_bin("agy") {
            let agy_qts = read_json_quota_ts(&agy_status_path());
            let last_try = LAST_AGY_PROBE_AT.load(Ordering::SeqCst);
            let needs_probe = force
                || ((now - agy_qts) >= STALE_QUOTA_THRESHOLD_SECS
                    && (now - last_try) >= MIN_AUTO_PROBE_INTERVAL_SECS);
            if needs_probe {
                LAST_AGY_PROBE_AT.store(now, Ordering::SeqCst);
                if run_pty_agy_refresh(&python_bin, &agy_bin) {
                    any_updated = true;
                }
            }
        }

        // 2. Claude live quota refresh (only if CLI session is authenticated)
        if let Some(claude_bin) = resolve_bin("claude") {
            let claude_qts = read_json_quota_ts(&claude_status_path()).max(read_claude_dotfile_ts());
            let last_try = LAST_CLAUDE_PROBE_AT.load(Ordering::SeqCst);
            let needs_probe = force
                || ((now - claude_qts) >= STALE_QUOTA_THRESHOLD_SECS
                    && (now - last_try) >= MIN_AUTO_PROBE_INTERVAL_SECS);
            if needs_probe {
                LAST_CLAUDE_PROBE_AT.store(now, Ordering::SeqCst);
                if is_claude_cli_logged_in(&claude_bin)
                    && run_pty_claude_refresh(&python_bin, &claude_bin)
                {
                    any_updated = true;
                }
            }
        }

        any_updated
    })
    .unwrap_or(false);

    PROBE_IN_PROGRESS.store(false, Ordering::SeqCst);
    result
}
