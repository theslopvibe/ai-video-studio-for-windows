//! Plain-text log files with local timestamps, one file per day, kept for 14 days.
//! API keys and large base64 payloads are scrubbed before anything is written.
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static DIR: OnceLock<PathBuf> = OnceLock::new();
static LAST: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
const KEEP_DAYS: u64 = 14;

pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = DIR.set(dir.to_path_buf());
    // drop old files
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs() > KEEP_DAYS * 86_400)
                .unwrap_or(false);
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

pub fn dir() -> Option<&'static Path> {
    DIR.get().map(|p| p.as_path())
}

/// Removes secrets and huge payloads from a message.
pub fn scrub(msg: &str) -> String {
    let mut s = msg.to_string();
    for name in crate::secrets::NAMES {
        if let Some(k) = crate::secrets::get(name) {
            if k.len() >= 8 {
                s = s.replace(&k, "<key>");
            }
        }
    }
    // long runs of base64-looking characters (images, videos)
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() > 200 {
            out.push_str(&format!("<data {} chars>", run.len()));
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    if out.chars().count() > 2000 {
        out = out.chars().take(2000).collect::<String>() + "…";
    }
    out.replace('\n', " ⏎ ")
}

pub fn write(level: &str, msg: &str) {
    let Some(dir) = DIR.get() else { return };
    let now = chrono::Local::now();
    let path = dir.join(format!("app-{}.log", now.format("%Y-%m-%d")));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} [{level}] {}", now.format("%Y-%m-%d %H:%M:%S%.3f"), scrub(msg));
    }
}

pub fn info(msg: &str) {
    write("INFO", msg)
}
#[allow(dead_code)]
pub fn warn(msg: &str) {
    write("WARN", msg)
}
pub fn error(msg: &str) {
    write("ERROR", msg)
}

/// Logs only when `value` differs from the last one logged under `key` (status changes, poll results).
pub fn on_change(key: &str, level: &str, msg: &str, value: &str) {
    let m = LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let mut g = m.lock().unwrap();
    if g.get(key).map(|v| v.as_str()) != Some(value) {
        g.insert(key.to_string(), value.to_string());
        drop(g);
        write(level, msg);
    }
}

/// The newest `n` lines across the latest log files (oldest first).
pub fn tail(n: usize) -> String {
    let Some(dir) = DIR.get() else { return String::new() };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "log").unwrap_or(false)).collect())
        .unwrap_or_default();
    files.sort();
    let mut lines: Vec<String> = vec![];
    for f in files.iter().rev() {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let mut chunk: Vec<String> = text.lines().map(String::from).collect();
        chunk.extend(lines);
        lines = chunk;
        if lines.len() >= n {
            break;
        }
    }
    let skip = lines.len().saturating_sub(n);
    lines[skip..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_hides_blobs_and_flattens_lines() {
        let blob = "A".repeat(500);
        let s = scrub(&format!("image {blob} ok\nnext"));
        assert!(s.contains("<data 500 chars>") && !s.contains("AAAAAAAAAA") && !s.contains('\n'));
        assert!(scrub("normal short text").contains("normal short text"));
    }
}
