//! Built-in chat: conversations in folders, search, and several providers (OpenAI-compatible APIs such as OpenAI,
//! GLM, DeepSeek, OpenRouter, Ollama; Gemini; Anthropic API; or a Claude subscription through the Claude Code CLI).
use crate::db::{now, Db};
use crate::{secrets, AppState};
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    /// openai | gemini | anthropic | cli (for cli, `base` is the program: claude | codex | gemini)
    pub kind: &'static str,
    pub base: &'static str,
    pub secret: &'static str,
    pub model: &'static str,
    /// Suggested models; the live list from the provider is added when it can be fetched.
    pub models: &'static [&'static str],
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "claude_sub", label: "Claude (subscription via Claude Code)", kind: "cli", base: "claude", secret: "", model: "sonnet", models: &["haiku", "sonnet", "opus"] },
    Preset { id: "chatgpt_sub", label: "ChatGPT (subscription via Codex CLI)", kind: "cli", base: "codex", secret: "", model: "", models: &["gpt-5", "gpt-5-codex"] },
    Preset { id: "gemini_sub", label: "Gemini (subscription via Gemini CLI)", kind: "cli", base: "gemini", secret: "", model: "", models: &["gemini-2.5-pro", "gemini-2.5-flash"] },
    Preset { id: "anthropic", label: "Claude (API key)", kind: "anthropic", base: "", secret: "anthropic_api_key", model: "claude-sonnet-5-5", models: &["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"] },
    Preset { id: "openai", label: "OpenAI GPT", kind: "openai", base: "https://api.openai.com/v1", secret: "openai_api_key", model: "gpt-4o-mini", models: &["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1", "o4-mini"] },
    Preset { id: "gemini", label: "Google Gemini", kind: "gemini", base: "", secret: "gemini_api_key", model: "gemini-2.5-flash", models: &["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-pro"] },
    Preset { id: "glm", label: "Z.ai GLM", kind: "openai", base: "https://api.z.ai/api/paas/v4", secret: "glm_api_key", model: "glm-5.1", models: &["glm-5.1", "glm-5", "glm-4.6", "glm-4.5-air"] },
    Preset { id: "glm_plan", label: "Z.ai GLM Coding Plan (subscription key)", kind: "openai", base: "https://api.z.ai/api/coding/paas/v4", secret: "glm_api_key", model: "glm-5.1", models: &["glm-5.1", "glm-5", "glm-4.6", "glm-4.5-air"] },
    Preset { id: "deepseek", label: "DeepSeek", kind: "openai", base: "https://api.deepseek.com", secret: "deepseek_api_key", model: "deepseek-chat", models: &["deepseek-chat", "deepseek-reasoner"] },
    Preset { id: "openrouter", label: "OpenRouter (many models)", kind: "openai", base: "https://openrouter.ai/api/v1", secret: "openrouter_api_key", model: "openai/gpt-4o-mini", models: &["openai/gpt-4o-mini", "anthropic/claude-sonnet-4.5", "google/gemini-2.5-flash", "deepseek/deepseek-chat", "z-ai/glm-4.6"] },
    Preset { id: "ollama", label: "Ollama (local, free)", kind: "openai", base: "http://localhost:11434/v1", secret: "", model: "llama3.2", models: &["llama3.2", "qwen2.5", "mistral"] },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn init(db: &Db) -> rusqlite::Result<()> {
    db.0.lock().unwrap().execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_folders(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS chats(id TEXT PRIMARY KEY, folder_id TEXT NOT NULL DEFAULT '', title TEXT NOT NULL,
           provider TEXT NOT NULL, model TEXT NOT NULL DEFAULT '', system TEXT NOT NULL DEFAULT '',
           pinned INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS chat_messages(id TEXT PRIMARY KEY, chat_id TEXT NOT NULL, role TEXT NOT NULL,
           content TEXT NOT NULL, created_at INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS chat_messages_chat ON chat_messages(chat_id, created_at);",
    )?;
    // older databases have no `mode`; the error for "duplicate column" is expected and ignored
    let _ = db.0.lock().unwrap().execute("ALTER TABLE chats ADD COLUMN mode TEXT NOT NULL DEFAULT 'chat'", []);
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
pub struct Folder {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Chat {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub provider: String,
    pub model: String,
    pub system: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// "chat" | "studio" (studio chats can queue videos and images)
    pub mode: String,
    /// the matching line when the list comes from a search
    pub snippet: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub model: &'static str,
    pub needs_key: bool,
    pub has_key: bool,
}

/// Model names for a provider: the suggestions plus whatever the provider says it has (Ollama: installed models).
/// Live lookups are best effort with a short timeout; failures just leave the suggestions.
pub async fn list_models(st: &AppState, provider: &str) -> Vec<String> {
    let Some(p) = preset(provider) else { return vec![] };
    let mut out: Vec<String> = vec![];
    let live: Option<Vec<String>> = async {
        let c = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().ok()?;
        match p.kind {
            "openai" => {
                let base = if p.id == "openai" { st.db.kv_get("openai_base_url").filter(|b| !b.trim().is_empty()).unwrap_or_else(|| p.base.into()) } else { p.base.into() };
                let (url, key) = if p.id == "ollama" { (format!("{}/models", base.trim_end_matches('/')), String::new()) } else { (format!("{}/models", base.trim_end_matches('/')), secrets::get(p.secret)?) };
                let mut rq = c.get(url);
                if !key.is_empty() {
                    rq = rq.bearer_auth(key);
                }
                let v: Value = rq.send().await.ok()?.json().await.ok()?;
                Some(v["data"].as_array()?.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            }
            "gemini" => {
                let key = secrets::get(p.secret)?;
                let v: Value = c.get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=100").header("x-goog-api-key", key).send().await.ok()?.json().await.ok()?;
                Some(
                    v["models"].as_array()?.iter()
                        .filter(|m| m["supportedGenerationMethods"].as_array().map(|a| a.iter().any(|x| x == "generateContent")).unwrap_or(false))
                        .filter_map(|m| m["name"].as_str().map(|n| n.trim_start_matches("models/").to_string()))
                        .collect(),
                )
            }
            "anthropic" => {
                let key = secrets::get(p.secret)?;
                let v: Value = c.get("https://api.anthropic.com/v1/models?limit=100").header("x-api-key", key).header("anthropic-version", "2023-06-01").send().await.ok()?.json().await.ok()?;
                Some(v["data"].as_array()?.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            }
            _ => None,
        }
    }
    .await;
    for m in live.unwrap_or_default().into_iter().chain(p.models.iter().map(|s| s.to_string())) {
        if !m.is_empty() && !out.contains(&m) {
            out.push(m);
        }
    }
    // suggestions first only when there is no live list; otherwise keep the live order but put the default on top
    out.retain(|m| m != p.model);
    out.insert(0, p.model.to_string());
    out.truncate(300);
    out
}

pub fn providers() -> Vec<ProviderInfo> {
    PRESETS
        .iter()
        .map(|p| ProviderInfo {
            id: p.id, label: p.label, model: p.model,
            needs_key: !p.secret.is_empty() || p.kind == "cli",
            has_key: if p.kind == "cli" { locate_cli(p.base).is_some() } else { p.secret.is_empty() || secrets::get(p.secret).is_some() },
        })
        .collect()
}

fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn chat_row(r: &rusqlite::Row) -> rusqlite::Result<Chat> {
    Ok(Chat {
        id: r.get(0)?, folder_id: r.get(1)?, title: r.get(2)?, provider: r.get(3)?, model: r.get(4)?, system: r.get(5)?,
        pinned: r.get::<_, i64>(6)? != 0, created_at: r.get(7)?, updated_at: r.get(8)?, mode: r.get(9)?, snippet: String::new(),
    })
}
const CHAT_COLS: &str = "id,folder_id,title,provider,model,system,pinned,created_at,updated_at,mode";

pub fn folders(db: &Db) -> Result<Vec<Folder>, String> {
    let c = db.0.lock().unwrap();
    let mut s = c.prepare("SELECT id,name FROM chat_folders ORDER BY name COLLATE NOCASE").map_err(|e| e.to_string())?;
    let rows = s.query_map([], |r| Ok(Folder { id: r.get(0)?, name: r.get(1)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

pub fn folder_create(db: &Db, name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Folder name is empty".into());
    }
    db.0.lock().unwrap().execute("INSERT INTO chat_folders(id,name,created_at) VALUES(?1,?2,?3)", params![uid(), name.trim(), now()]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn folder_rename(db: &Db, id: &str, name: &str) -> Result<(), String> {
    db.0.lock().unwrap().execute("UPDATE chat_folders SET name=?2 WHERE id=?1", params![id, name.trim()]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Chats in a deleted folder are kept and move to "no folder".
pub fn folder_delete(db: &Db, id: &str) -> Result<(), String> {
    let c = db.0.lock().unwrap();
    c.execute("UPDATE chats SET folder_id='' WHERE folder_id=?1", params![id]).map_err(|e| e.to_string())?;
    c.execute("DELETE FROM chat_folders WHERE id=?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

fn like(q: &str) -> String {
    format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

/// All chats, pinned first then newest. With `query`: chats whose title or any message contains it, plus a snippet.
pub fn chats(db: &Db, query: &str) -> Result<Vec<Chat>, String> {
    let c = db.0.lock().unwrap();
    let q = query.trim();
    if q.is_empty() {
        let mut s = c.prepare(&format!("SELECT {CHAT_COLS} FROM chats ORDER BY pinned DESC, updated_at DESC")).map_err(|e| e.to_string())?;
        let rows = s.query_map([], chat_row).map_err(|e| e.to_string())?;
        return rows.collect::<Result<_, _>>().map_err(|e| e.to_string());
    }
    let pat = like(q);
    let mut s = c
        .prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats WHERE title LIKE ?1 ESCAPE '\\' OR id IN (SELECT chat_id FROM chat_messages WHERE content LIKE ?1 ESCAPE '\\') ORDER BY pinned DESC, updated_at DESC LIMIT 100"
        ))
        .map_err(|e| e.to_string())?;
    let mut out: Vec<Chat> = s.query_map(params![pat], chat_row).map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    for ch in out.iter_mut() {
        let text: Option<String> = c
            .query_row("SELECT content FROM chat_messages WHERE chat_id=?1 AND content LIKE ?2 ESCAPE '\\' ORDER BY created_at LIMIT 1", params![ch.id, pat], |r| r.get(0))
            .ok();
        if let Some(t) = text {
            ch.snippet = snippet(&t, q);
        }
    }
    Ok(out)
}

/// ~120 characters of `text` around the first (case-insensitive) hit of `q`.
pub fn snippet(text: &str, q: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = q.to_lowercase().chars().collect();
    let pos = if lower.len() == chars.len() { lower.windows(needle.len().max(1)).position(|w| w == needle.as_slice()).unwrap_or(0) } else { 0 };
    let start = pos.saturating_sub(40);
    let end = (pos + needle.len() + 80).min(chars.len());
    let s: String = chars[start..end].iter().collect::<String>().replace('\n', " ");
    format!("{}{}{}", if start > 0 { "…" } else { "" }, s.trim(), if end < chars.len() { "…" } else { "" })
}

pub fn chat_create(db: &Db, folder_id: &str, provider: &str, model: &str, mode: &str) -> Result<Chat, String> {
    let p = preset(provider).ok_or_else(|| format!("Unknown provider {provider}"))?;
    let id = uid();
    let t = now();
    let model = if model.trim().is_empty() { p.model } else { model.trim() };
    let mode = if mode == "studio" { "studio" } else { "chat" };
    db.0.lock().unwrap()
        .execute("INSERT INTO chats(id,folder_id,title,provider,model,system,pinned,created_at,updated_at,mode) VALUES(?1,?2,'New chat',?3,?4,'',0,?5,?5,?6)", params![id, folder_id, provider, model, t, mode])
        .map_err(|e| e.to_string())?;
    chat(db, &id).ok_or_else(|| "chat not created".into())
}

pub fn chat(db: &Db, id: &str) -> Option<Chat> {
    db.0.lock().unwrap().query_row(&format!("SELECT {CHAT_COLS} FROM chats WHERE id=?1"), params![id], chat_row).ok()
}

pub fn chat_update(db: &Db, id: &str, title: &str, folder_id: &str, provider: &str, model: &str, system: &str, pinned: bool) -> Result<(), String> {
    if preset(provider).is_none() {
        return Err(format!("Unknown provider {provider}"));
    }
    db.0.lock().unwrap()
        .execute("UPDATE chats SET title=?2, folder_id=?3, provider=?4, model=?5, system=?6, pinned=?7 WHERE id=?1", params![id, title.trim(), folder_id, provider, model.trim(), system, pinned as i64])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn chat_delete(db: &Db, id: &str) -> Result<(), String> {
    let c = db.0.lock().unwrap();
    c.execute("DELETE FROM chat_messages WHERE chat_id=?1", params![id]).map_err(|e| e.to_string())?;
    c.execute("DELETE FROM chats WHERE id=?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn messages(db: &Db, chat_id: &str) -> Result<Vec<Message>, String> {
    let c = db.0.lock().unwrap();
    let mut s = c.prepare("SELECT id,role,content,created_at FROM chat_messages WHERE chat_id=?1 ORDER BY created_at, rowid").map_err(|e| e.to_string())?;
    let rows = s.query_map(params![chat_id], |r| Ok(Message { id: r.get(0)?, role: r.get(1)?, content: r.get(2)?, created_at: r.get(3)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

fn add_message(db: &Db, chat_id: &str, role: &str, content: &str) -> Result<(), String> {
    let c = db.0.lock().unwrap();
    c.execute("INSERT INTO chat_messages(id,chat_id,role,content,created_at) VALUES(?1,?2,?3,?4,?5)", params![uid(), chat_id, role, content, now()]).map_err(|e| e.to_string())?;
    c.execute("UPDATE chats SET updated_at=?2 WHERE id=?1", params![chat_id, now()]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn message_delete(db: &Db, id: &str) -> Result<(), String> {
    db.0.lock().unwrap().execute("DELETE FROM chat_messages WHERE id=?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Adds the user's message, asks the provider, stores and returns the whole conversation.
pub async fn send(st: &AppState, chat_id: &str, text: &str) -> Result<Vec<Message>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Write a message first".into());
    }
    let ch = chat(&st.db, chat_id).ok_or("Chat not found")?;
    add_message(&st.db, chat_id, "user", text)?;
    if ch.title == "New chat" {
        let t: String = text.chars().take(48).collect();
        st.db.0.lock().unwrap().execute("UPDATE chats SET title=?2 WHERE id=?1", params![chat_id, t.replace('\n', " ")]).map_err(|e| e.to_string())?;
    }
    let history: Vec<(String, String)> = messages(&st.db, chat_id)?.into_iter().map(|m| (m.role, m.content)).collect();
    let system = if ch.mode == "studio" { format!("{}
{}", studio_system(st), ch.system) } else { ch.system.clone() };
    let reply = complete(st, &ch.provider, &ch.model, &system, &history).await?;
    add_message(&st.db, chat_id, "assistant", &reply)?;
    messages(&st.db, chat_id)
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(180)).build().map_err(|e| e.to_string())
}

fn api_error(who: &str, code: reqwest::StatusCode, v: &Value, raw: &str) -> String {
    let m = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).or_else(|| v["message"].as_str()).map(String::from).unwrap_or_else(|| raw.chars().take(200).collect());
    match code.as_u16() {
        401 | 403 => format!("{who}: the key was rejected. Check it in Settings. ({m})"),
        429 => format!("{who}: rate limit or no credit left. ({m})"),
        _ => format!("{who} {code}: {m}"),
    }
}

pub async fn complete(st: &AppState, provider: &str, model: &str, system: &str, history: &[(String, String)]) -> Result<String, String> {
    let p = preset(provider).ok_or_else(|| format!("Unknown provider {provider}"))?;
    let model = if model.trim().is_empty() { p.model } else { model };
    let key = if p.secret.is_empty() { String::new() } else { secrets::get(p.secret).ok_or_else(|| format!("{} needs an API key. Add it in Settings → Chat providers.", p.label))? };
    let c = http()?;
    match p.kind {
        "openai" => {
            let base = if p.id == "openai" { st.db.kv_get("openai_base_url").filter(|b| !b.trim().is_empty()).unwrap_or_else(|| p.base.into()) } else { p.base.into() };
            let mut msgs = vec![];
            if !system.trim().is_empty() {
                msgs.push(json!({"role": "system", "content": system}));
            }
            msgs.extend(history.iter().map(|(r, t)| json!({"role": r, "content": t})));
            let mut rq = c.post(format!("{}/chat/completions", base.trim_end_matches('/'))).json(&json!({"model": model, "messages": msgs}));
            if !key.is_empty() {
                rq = rq.bearer_auth(&key);
            }
            let r = rq.send().await.map_err(|e| format!("{}: {e}", p.label))?;
            let code = r.status();
            let raw = r.text().await.map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            if !code.is_success() {
                return Err(api_error(p.label, code, &v, &raw));
            }
            let t = v["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
            if t.is_empty() { Err(format!("{}: empty answer", p.label)) } else { Ok(t) }
        }
        "anthropic" => {
            let msgs: Vec<Value> = history.iter().map(|(r, t)| json!({"role": r, "content": t})).collect();
            let mut body = json!({"model": model, "max_tokens": 4096, "messages": msgs});
            if !system.trim().is_empty() {
                body["system"] = json!(system);
            }
            let r = c.post("https://api.anthropic.com/v1/messages").header("x-api-key", &key).header("anthropic-version", "2023-06-01").json(&body).send().await.map_err(|e| format!("{}: {e}", p.label))?;
            let code = r.status();
            let raw = r.text().await.map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            if !code.is_success() {
                return Err(api_error(p.label, code, &v, &raw));
            }
            let t = v["content"].as_array().map(|a| a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default();
            if t.trim().is_empty() { Err(format!("{}: empty answer", p.label)) } else { Ok(t.trim().into()) }
        }
        "gemini" => {
            let contents: Vec<Value> = history.iter().map(|(r, t)| json!({"role": if r == "assistant" { "model" } else { "user" }, "parts": [{"text": t}]})).collect();
            let mut body = json!({"contents": contents});
            if !system.trim().is_empty() {
                body["systemInstruction"] = json!({"parts": [{"text": system}]});
            }
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
            let r = c.post(url).header("x-goog-api-key", &key).json(&body).send().await.map_err(|e| format!("{}: {e}", p.label))?;
            let code = r.status();
            let raw = r.text().await.map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            if !code.is_success() {
                return Err(api_error(p.label, code, &v, &raw));
            }
            let t = v["candidates"][0]["content"]["parts"].as_array().map(|a| a.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default();
            if t.trim().is_empty() { Err(format!("{}: empty answer ({})", p.label, v["candidates"][0]["finishReason"].as_str().unwrap_or("?"))) } else { Ok(t.trim().into()) }
        }
        "cli" => {
            let transcript = cli_transcript(history);
            let sys = if system.trim().is_empty() { "You are a helpful assistant in a chat window. Answer the last user message." } else { system };
            run_cli(p.base, sys, &transcript, model).await
        }
        other => Err(format!("Unsupported provider kind {other}")),
    }
}


/// Finds a subscription CLI (`claude`, `codex`, `gemini`): PATH first, then the usual npm and install folders.
pub fn locate_cli(name: &str) -> Option<std::path::PathBuf> {
    if name == "claude" {
        return crate::anthropic::locate_cli();
    }
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(a) = std::env::var_os("APPDATA") {
        dirs.push(std::path::PathBuf::from(a).join("npm"));
    }
    if let Some(h) = std::env::var_os("USERPROFILE") {
        dirs.push(std::path::PathBuf::from(h).join(".local").join("bin"));
    }
    dirs.into_iter().flat_map(|d| [d.join(format!("{name}.exe")), d.join(format!("{name}.cmd"))]).find(|p| p.is_file())
}

/// One answer from a subscription CLI. Claude keeps its own system-prompt flag; Codex and Gemini get the
/// instructions at the top of the prompt, which they read from stdin.
async fn run_cli(tool: &str, system: &str, input: &str, model: &str) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    if tool == "claude" {
        return crate::anthropic::run_cli(system, input, model).await.map(|o| o.text);
    }
    let exe = locate_cli(tool).ok_or_else(|| match tool {
        "codex" => "Codex CLI not found. Install it (npm i -g @openai/codex), run `codex` once and sign in with your ChatGPT account.".to_string(),
        _ => "Gemini CLI not found. Install it (npm i -g @google/gemini-cli), run `gemini` once and sign in with your Google account.".to_string(),
    })?;
    let prompt = format!("{system}

---

{input}");
    // a fresh empty folder per call: the CLI sees no project files, settings or docs from a shared temp folder
    let work = std::env::temp_dir().join(format!("avs-cli-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let out_file = work.join("answer.txt");
    let mut cmd = tokio::process::Command::new(exe);
    if tool == "codex" {
        cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "project_doc_max_bytes=0", "--output-last-message"]).arg(&out_file);
        if !model.trim().is_empty() {
            cmd.args(["-m", model.trim()]);
        }
        cmd.arg("-");
    } else {
        cmd.args(["-p", "Answer the request given on stdin. Do not use any tools.", "--approval-mode", "default"]);
        if !model.trim().is_empty() {
            cmd.args(["-m", model.trim()]);
        }
    }
    cmd.current_dir(&work).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| format!("{tool}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("stdin")?;
    stdin.write_all(prompt.as_bytes()).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let out = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output()).await.map_err(|_| format!("{tool}: timeout (180s)"))?.map_err(|e| e.to_string())?;
    let text = if tool == "codex" { std::fs::read_to_string(&out_file).unwrap_or_default() } else { String::from_utf8_lossy(&out.stdout).into_owned() };
    let _ = std::fs::remove_dir_all(&work);
    let text = text.trim().to_string();
    if !out.status.success() || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() { text } else { err.trim().to_string() };
        return Err(format!("{tool}: {}", msg.chars().take(300).collect::<String>()));
    }
    Ok(text)
}

/// Instructions for "Create video with chat". The catalogue is inlined so the model needs no tool calls and
/// the same text works for APIs and subscription CLIs.
/// Imported workflow files are untrusted: one short line, no control characters, so they cannot add instructions.
fn plain(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(160).collect()
}

pub fn studio_system(st: &AppState) -> String {
    let (wfs, _) = st.load_workflows();
    let mut cat = String::new();
    let cloud = crate::cloud_provider(st);
    for w in wfs.iter() {
        let m = &w.meta;
        // the other cloud's modes are not offered: only the cloud chosen in Settings works
        if m.provider == "runpod" && cloud != "runpod" { continue; }
        let keys: Vec<&str> = m.inputs.iter().map(|i| i.key.as_str()).filter(|k| !matches!(*k, "prompt" | "negative" | "seed") && !k.starts_with("image") && !k.starts_with("audio") && !k.starts_with("video")).collect();
        let needs: Vec<String> = m.inputs.iter().filter(|i| matches!(i.ty.as_str(), "image" | "audio" | "video")).map(|i| format!("{}:{}", i.key, i.ty)).collect();
        let langs = if m.languages.is_empty() { String::new() } else { format!(" | languages: {}", m.languages.join(", ")) };
        let cost = if m.provider.is_empty() && cloud == "vast" { format!("open source, runs on the user's PC or a rented Vast card, ~{}s", m.est_sec) } else if m.provider.is_empty() { format!("open source on the user's PC, ~{}s, free", m.est_sec) } else if m.provider == "runpod" { format!("RunPod cloud GPU (wakes up by itself), ~${:.2} per result", m.est_usd) } else { format!("paid API, ~${:.2} per result", m.est_usd) };
        cat.push_str(&format!("- {} | {} | {} | {} | good for: {} | params: {} | inputs: {}{}
", plain(&m.id), plain(&m.name_en), plain(&m.kind), cost, plain(&m.good_en), plain(&keys.join(",")), if needs.is_empty() { "none".into() } else { plain(&needs.join(",")) }, plain(&langs)));
    }
    format!(
        "You are the assistant inside AI Video Studio, an app that makes videos and images from prompts. Reply in the user's language, briefly. Ask at most one short question and only if essential. Recommend a model for the task (one line: why, speed, price). Write the prompt in English: subject, one action, setting, light, camera, style, one continuous shot.
To queue a result add one block per clip, exactly:
```video
{{\"workflow\":\"<id>\",\"prompt\":\"<English prompt or the text to speak>\",\"params\":{{}}}}
```
Steps can be chained: give a block an \"id\" (e.g. \"a\") and in a later block set \"inputs\":{{\"image\":\"@a\"}} to feed it the result of block a (keys come from that model's \"inputs\"; use \"@last\" for the user's most recent finished result of the right type). Typical chains: picture (t2i) -> video (i2v) ; text -> voice (tts) -> lip-sync (lipsync, needs a video with a face + the voice) ; video -> smoother (v2v). If a step needs a file only the user has (a voice sample, a recording, a photo), leave it out of \"inputs\": the card asks them to choose it. Before suggesting voice cloning, voice change or lip-sync on a real person, remind the user to use only their own voice/face or one they have permission for. When asked about languages, answer from the \"languages\" field. If nothing in the list fits, say so; never invent ids.
The app shows the user a card with the price and a Generate button, so never say it is already generating. Use only ids from the list; put only keys from that model's params in \"params\" (e.g. width, height, length, fps) and leave it empty when unsure. For several scenes write several blocks in order. For music pick a t2a model: prompt = style tags, params lyrics (or [instrumental]) and seconds. Image-to-video needs a start image: tell the user to use the Generate page for that.
Default run mode: {}.
Models (id | name | kind | cost | use | params):
{}",
        if st.db.kv_get("provider").as_deref() == Some("local") { "this PC (open-source modes)" } else if cloud == "vast" { "Vast cloud card (the open-source modes on a rented card)" } else { "RunPod cloud (the RunPod modes)" },
        cat
    )
}

/// The CLIs take one prompt, so the conversation is flattened into a transcript.
pub fn cli_transcript(history: &[(String, String)]) -> String {
    if history.len() == 1 {
        return history[0].1.clone();
    }
    let mut s = String::from("Conversation so far:\n\n");
    for (r, t) in history {
        s.push_str(if r == "assistant" { "Assistant: " } else { "User: " });
        s.push_str(t);
        s.push_str("\n\n");
    }
    s.push_str("Reply as the Assistant to the last User message. Output only the reply.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        let d = Db(std::sync::Mutex::new(c));
        init(&d).unwrap();
        d
    }

    #[test]
    fn folders_chats_and_search() {
        let d = db();
        folder_create(&d, "Ideas").unwrap();
        let f = folders(&d).unwrap().remove(0);
        let a = chat_create(&d, &f.id, "openai", "", "chat").unwrap();
        let b = chat_create(&d, "", "glm", "glm-5.1", "chat").unwrap();
        assert_eq!(a.model, "gpt-4o-mini");
        add_message(&d, &a.id, "user", "Write a poem about foxes in the snow").unwrap();
        add_message(&d, &b.id, "user", "100% sure? use_underscore").unwrap();
        let hits = chats(&d, "FOXES").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a.id);
        assert!(hits[0].snippet.to_lowercase().contains("foxes"));
        // % and _ are searched literally
        assert_eq!(chats(&d, "100%").unwrap().len(), 1);
        assert_eq!(chats(&d, "use_u").unwrap().len(), 1);
        assert_eq!(chats(&d, "%").unwrap().len(), 1);
        folder_delete(&d, &f.id).unwrap();
        assert_eq!(chat(&d, &a.id).unwrap().folder_id, "");
        chat_delete(&d, &a.id).unwrap();
        assert!(messages(&d, &a.id).unwrap().is_empty());
        assert_eq!(chats(&d, "").unwrap().len(), 1);
    }

    #[test]
    fn pinned_chats_come_first() {
        let d = db();
        let a = chat_create(&d, "", "openai", "", "chat").unwrap();
        let b = chat_create(&d, "", "openai", "", "chat").unwrap();
        chat_update(&d, &a.id, "Old but pinned", "", "openai", "", "", true).unwrap();
        d.0.lock().unwrap().execute("UPDATE chats SET updated_at=updated_at+100 WHERE id=?1", params![b.id]).unwrap();
        assert_eq!(chats(&d, "").unwrap()[0].id, a.id);
    }

    #[test]
    fn transcript_and_snippet() {
        assert_eq!(cli_transcript(&[("user".into(), "hi".into())]), "hi");
        let t = cli_transcript(&[("user".into(), "a".into()), ("assistant".into(), "b".into()), ("user".into(), "c".into())]);
        assert!(t.contains("Assistant: b") && t.ends_with("Output only the reply."));
        assert!(snippet(&"x".repeat(200), "x").ends_with('…'));
    }

    #[test]
    fn studio_chats_keep_their_mode() {
        let d = db();
        assert_eq!(chat_create(&d, "", "openai", "", "studio").unwrap().mode, "studio");
        assert_eq!(chat_create(&d, "", "openai", "", "whatever").unwrap().mode, "chat");
    }

    #[test]
    fn presets_are_consistent() {
        for p in PRESETS {
            assert!(!p.model.is_empty() || p.kind == "cli", "{}", p.id);
            if p.kind == "openai" {
                assert!(p.base.starts_with("http"), "{}", p.id);
            }
            if p.kind == "cli" {
                assert!(matches!(p.base, "claude" | "codex" | "gemini"), "{}", p.id);
            }
        }
        assert!(preset("glm").is_some() && preset("nope").is_none());
    }
}
