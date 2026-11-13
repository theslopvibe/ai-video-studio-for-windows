//! `app.exe --mcp`: the same Claude Desktop connector as mcp/server/index.js, but built into the program.
//! Claude Desktop starts it from claude_desktop_config.json (no extension file and no Node.js needed).
//! It speaks MCP over stdin/stdout and forwards every tool to the running app's local API (port + token in api.json).
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const TOOLS: &str = include_str!("../mcp_tools.json");

fn api_info() -> Result<(u16, String), String> {
    let base = std::env::var("AIVS_DATA_DIR").unwrap_or_else(|_| format!("{}\\com.aivideostudio.app", std::env::var("APPDATA").unwrap_or_default()));
    let text = std::fs::read_to_string(format!("{base}\\api.json")).map_err(|_| closed_msg())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| closed_msg())?;
    Ok((v["port"].as_u64().unwrap_or(0) as u16, v["token"].as_str().unwrap_or("").to_string()))
}

fn closed_msg() -> String {
    "Δεν βρέθηκε σύνδεση με το AI Video Studio For Windows (είτε το πρόγραμμα είναι κλειστό, είτε η σύνδεση με το Claude Desktop είναι απενεργοποιημένη στις Ρυθμίσεις του). Πες στον χρήστη να κάνει ΜΕ ΤΗ ΣΕΙΡΑ: 1) να ανοίξει το πρόγραμμα «AI Video Studio For Windows» και να το αφήσει ανοιχτό, 2) στο πρόγραμμα: Ρυθμίσεις (Settings) → κουτί «Claude Desktop connection» → να τσεκάρει το «Allow the Claude Desktop connection», 3) να ξαναζητήσει το ίδιο από εδώ.".into()
}

async fn call(c: &reqwest::Client, method: &str, route: &str, body: Option<Value>) -> Result<Value, String> {
    let (port, token) = api_info()?;
    let m = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut rb = c.request(m, format!("http://127.0.0.1:{port}{route}")).bearer_auth(token);
    if let Some(b) = body {
        rb = rb.json(&b);
    }
    let r = rb.send().await.map_err(|_| closed_msg())?;
    let code = r.status();
    let text = r.text().await.map_err(|e| e.to_string())?;
    let data: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "error": text }));
    if code.as_u16() == 404 {
        return Err("Το πρόγραμμα απέρριψε το αίτημα (παλιός κωδικός σύνδεσης). Πες στον χρήστη να κλείσει εντελώς και να ανοίξει ξανά το AI Video Studio For Windows, και μετά ξαναδοκίμασε.".into());
    }
    if !code.is_success() {
        return Err(data["error"].as_str().map(String::from).unwrap_or_else(|| format!("HTTP {code}")));
    }
    Ok(data)
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

async fn run_tool(c: &reqwest::Client, name: &str, a: &Value) -> Result<Value, String> {
    let confirmed = a["user_confirmed"] == json!(true);
    match name {
        "video_studio_status" => call(c, "GET", "/v1/status", None).await,
        "list_workflows" => call(c, "GET", "/v1/workflows", None).await,
        "runpod_status" => call(c, "GET", "/v1/runpod/status", None).await,
        "runpod_wake" => call(c, "POST", "/v1/runpod/wake", Some(json!({}))).await,
        "runpod_sleep" => call(c, "POST", "/v1/runpod/sleep", Some(json!({}))).await,
        "generate_video" => {
            if !confirmed {
                return Err("Ρώτα πρώτα τον χρήστη (με κόστος) και ξανακάλεσε με user_confirmed=true.".into());
            }
            let mut params = a["extra_params"].as_object().cloned().unwrap_or_default();
            for k in ["width", "height", "length", "fps", "seed", "negative"] {
                if !a[k].is_null() {
                    params.insert(k.into(), a[k].clone());
                }
            }
            let mut body = json!({ "params": params });
            for k in ["prompt", "kind", "provider", "image_path", "end_image_path", "workflow_id", "project", "scene", "inputs"] {
                if !a[k].is_null() {
                    body[k] = a[k].clone();
                }
            }
            call(c, "POST", "/v1/jobs", Some(body)).await
        }
        "set_defaults" => call(c, "POST", "/v1/settings", Some(json!({ "model": a["model"], "provider": a["provider"], "auto_sleep_min": a["auto_sleep_min"], "runpod_instance": a["runpod_instance"] }))).await,
        "get_job" => {
            let id = a["job_id"].as_str().ok_or("job_id is required")?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(a["wait_seconds"].as_u64().unwrap_or(0).min(45));
            loop {
                let j = call(c, "GET", &format!("/v1/jobs/{}", enc(id)), None).await?;
                let done = matches!(j["status"].as_str(), Some("done" | "failed" | "cancelled"));
                if done || std::time::Instant::now() >= until {
                    return Ok(j);
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
        "list_jobs" => call(c, "GET", "/v1/jobs", None).await,
        "cancel_job" => call(c, "POST", &format!("/v1/jobs/{}/cancel", enc(a["job_id"].as_str().ok_or("job_id is required")?)), Some(json!({}))).await,
        "retry_job" => call(c, "POST", &format!("/v1/jobs/{}/retry", enc(a["job_id"].as_str().ok_or("job_id is required")?)), Some(json!({}))).await,
        "make_report" => call(c, "POST", "/v1/report", Some(json!({}))).await,
        _ => Err(format!("Unknown tool {name}")),
    }
}

pub fn run() {
    let defs: Value = serde_json::from_str(TOOLS).unwrap_or_else(|_| json!({ "instructions": "", "tools": [] }));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let client = reqwest::Client::new();
    let stdout = std::io::stdout();
    let send = |v: Value| {
        let mut o = stdout.lock();
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    };
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let (id, method, params) = (msg["id"].clone(), msg["method"].as_str().unwrap_or("").to_string(), msg["params"].clone());
        if id.is_null() {
            continue;
        }
        match method.as_str() {
            "initialize" => send(json!({ "jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2024-11-05"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "ai-video-studio", "version": env!("CARGO_PKG_VERSION") },
                "instructions": defs["instructions"] } })),
            "ping" => send(json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
            "tools/list" => send(json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": defs["tools"] } })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                let out = rt.block_on(run_tool(&client, &name, &params["arguments"]));
                send(match out {
                    Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] } }),
                    Err(e) => json!({ "jsonrpc": "2.0", "id": id, "result": { "isError": true, "content": [{ "type": "text", "text": e }] } }),
                });
            }
            "resources/list" => send(json!({ "jsonrpc": "2.0", "id": id, "result": { "resources": [] } })),
            "prompts/list" => send(json!({ "jsonrpc": "2.0", "id": id, "result": { "prompts": [] } })),
            _ => send(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Method not found: {method}") } })),
        }
    }
}
