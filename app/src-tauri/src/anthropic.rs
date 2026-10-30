use serde_json::{json, Value};
use std::time::Duration;

pub const MODELS: &[&str] = &["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"];
pub const CLI_MODELS: &[&str] = &["haiku", "sonnet", "opus"];

/// Kept short on purpose: every token of it is paid on each call.
pub const SYSTEM: &str = "Write ONE English text-to-video prompt (Wan 2.2), 50-70 words, from the user's idea (any language). Cover: subject details, one action, setting, light, camera shot and movement, style. One continuous shot. Visual only: no text, logos or sound. Stay faithful to the idea. Output only the prompt.";

pub async fn enhance(key: &str, model: &str, idea: &str, hint: &str) -> Result<crate::llm::Out, String> {
    let c = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let system = if hint.trim().is_empty() { SYSTEM.to_string() } else { format!("{SYSTEM}\nExtra context about the target workflow: {hint}") };
    let r = c
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model, "max_tokens": 250, "system": system,
            "messages": [{"role": "user", "content": idea}]
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let code = r.status();
    let v: Value = r.json().await.map_err(|e| e.to_string())?;
    if !code.is_success() {
        let m = v["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("Anthropic {code}: {m}"));
    }
    let text = v["content"]
        .as_array()
        .map(|a| a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("Empty answer from Anthropic".into());
    }
    let tokens = v["usage"]["input_tokens"].as_u64().unwrap_or(0) + v["usage"]["output_tokens"].as_u64().unwrap_or(0);
    Ok(crate::llm::Out { text: text.trim().trim_matches('"').to_string(), tokens: Some(tokens) })
}

/// Cheapest possible call that proves the key and model work.
pub async fn test(key: &str, model: &str) -> Result<(), String> {
    let c = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())?;
    let r = c
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({"model": model, "max_tokens": 5, "messages": [{"role": "user", "content": "ping"}]}))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if r.status().is_success() {
        return Ok(());
    }
    let code = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    Err(format!("Anthropic {code}: {}", v["error"]["message"].as_str().unwrap_or("")))
}

// ---- Claude Code CLI (uses the user's Claude subscription login, no API key) ----

/// Finds the `claude` executable: PATH first, then the usual install locations.
pub fn locate_cli() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(h) = std::env::var_os("USERPROFILE") {
        dirs.push(std::path::PathBuf::from(&h).join(".local").join("bin"));
        dirs.push(std::path::PathBuf::from(&h).join(".claude").join("local"));
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        dirs.push(std::path::PathBuf::from(a).join("npm"));
    }
    for d in dirs {
        for n in ["claude.exe", "claude.cmd"] {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub async fn run_cli(system: &str, input: &str, model: &str) -> Result<crate::llm::Out, String> {
    use tokio::io::AsyncWriteExt;
    let exe = locate_cli().ok_or("Δεν βρέθηκε το Claude Code (claude). Εγκατάστησέ το από claude.com/claude-code και κάνε login με τη συνδρομή σου.")?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(["-p", "--system-prompt", &system.replace('\n', " "), "--tools", "", "--no-session-persistence", "--setting-sources", "", "--model", model, "--output-format", "json"])
        .current_dir(std::env::temp_dir())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| format!("Claude Code: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("stdin")?;
    stdin.write_all(input.as_bytes()).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let out = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| "Claude Code: timeout (120s)".to_string())?
        .map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let text = v["result"].as_str().unwrap_or("").trim().to_string();
    if !out.status.success() || v["is_error"] == true || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if !text.is_empty() { text } else if !err.trim().is_empty() { err.trim().to_string() } else { raw };
        let hint = if msg.to_lowercase().contains("login") || msg.to_lowercase().contains("auth") {
            " — τρέξε `claude` σε terminal και κάνε login."
        } else { "" };
        return Err(format!("Claude Code: {}{hint}", msg.chars().take(300).collect::<String>()));
    }
    let u = &v["usage"];
    let tokens = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens"]
        .iter()
        .map(|k| u[k].as_u64().unwrap_or(0))
        .sum::<u64>();
    Ok(crate::llm::Out { text, tokens: Some(tokens) })
}

pub async fn enhance_cli(model: &str, idea: &str, hint: &str) -> Result<crate::llm::Out, String> {
    let system = if hint.trim().is_empty() { SYSTEM.to_string() } else { format!("{SYSTEM} Extra context about the target workflow: {hint}") };
    run_cli(&system, idea, model).await.map(|mut o| { o.text = o.text.trim_matches('"').to_string(); o })
}

#[cfg(test)]
mod tests {
    /// Needs a logged-in Claude Code: `cargo test -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn cli_enhance_end_to_end() {
        let o = super::enhance_cli("haiku", "ένας γάτος στη βροχή", "").await.unwrap();
        println!("PATH={:?}\nTEXT={}\nTOKENS={:?}", super::locate_cli(), o.text, o.tokens);
        assert!(o.text.split_whitespace().count() > 15);
        assert!(o.tokens.unwrap() > 0);
    }
}
