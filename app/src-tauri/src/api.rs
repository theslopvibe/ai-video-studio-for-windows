//! Local HTTP API on 127.0.0.1 so an MCP server (Claude Desktop) can drive the app.
//! Bearer token + port are written to `<data dir>/api.json`; the MCP reads that file.
use crate::{db, gpu_offers, job_cancel, job_enqueue, job_retry, logs_report, secrets, AppState};
use serde_json::{json, Map, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PREFERRED_PORT: u16 = 47615;
const MAX_BODY: usize = 24 * 1024 * 1024;

pub async fn serve(app: AppHandle) {
    let st = app.state::<AppState>();
    let token = match st.db.kv_get("api_token") {
        Some(t) if t.len() >= 32 => t,
        _ => {
            let t = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
            let _ = st.db.kv_set("api_token", &t);
            t
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", PREFERRED_PORT)).await {
        Ok(l) => l,
        Err(_) => match TcpListener::bind(("127.0.0.1", 0)).await {
            Ok(l) => l,
            Err(x) => {
                crate::log::error(&format!("api: bind failed: {x}"));
                return;
            }
        },
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(PREFERRED_PORT);
    let info = json!({ "port": port, "token": token, "version": env!("CARGO_PKG_VERSION") });
    if let Err(x) = std::fs::write(st.data_dir.join("api.json"), info.to_string()) {
        crate::log::error(&format!("api: cannot write api.json: {x}"));
    }
    crate::log::info(&format!("api: listening on 127.0.0.1:{port}"));
    loop {
        let Ok((sock, _)) = listener.accept().await else { continue };
        let app = app.clone();
        let token = token.clone();
        tauri::async_runtime::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(60), handle(sock, app, token)).await;
        });
    }
}

struct Req {
    method: String,
    path: String,
    query: String,
    auth: String,
    host_ok: bool,
    body: Vec<u8>,
}

/// Compares without stopping at the first differing byte.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

/// Only requests addressed to this machine (blocks DNS rebinding from a web page).
fn local_host(v: &str) -> bool {
    let h = v.trim().to_ascii_lowercase();
    h == "127.0.0.1" || h == "localhost" || h.starts_with("127.0.0.1:") || h.starts_with("localhost:")
}

async fn read_req(sock: &mut TcpStream) -> Option<Req> {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 64 * 1024 {
            return None;
        }
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let (path, query) = target.split_once('?').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((target, String::new()));
    let mut auth = String::new();
    let mut host_ok = false;
    let mut len = 0usize;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "authorization" => auth = v.trim().to_string(),
                "host" => host_ok = local_host(v),
                "content-length" => len = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    if len > MAX_BODY {
        return None;
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(len);
    Some(Req { method, path, query, auth, host_ok, body })
}

async fn handle(mut sock: TcpStream, app: AppHandle, token: String) {
    let Some(req) = read_req(&mut sock).await else { return };
    // wrong host or token: an empty 404, nothing that identifies the app
    let authed = req.host_ok && req.auth.strip_prefix("Bearer ").is_some_and(|t| same_secret(t.trim(), &token));
    if !authed {
        let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
        let _ = sock.shutdown().await;
        return;
    }
    let (code, body) = {
        match route(&app, &req).await {
            Ok(v) => (200, v),
            Err(m) => (400, json!({ "error": m })),
        }
    };
    let text = body.to_string();
    let reason = if code == 200 { "OK" } else if code == 401 { "Unauthorized" } else { "Bad Request" };
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
    let _ = sock.write_all(resp.as_bytes()).await;
    let _ = sock.shutdown().await;
}

fn query_f64(q: &str, key: &str) -> Option<f64> {
    q.split('&').filter_map(|p| p.split_once('=')).find(|(k, _)| *k == key).and_then(|(_, v)| v.parse().ok())
}

fn job_json(j: &db::Job) -> Value {
    json!({
        "id": j.id, "status": j.status, "message": j.message, "error": j.error, "file_path": j.file_path,
        "prompt": j.prompt, "workflow_id": j.workflow_id, "gpu": j.gpu, "est_cost_usd": j.est_cost,
        "created_at": j.created_at, "started_at": j.started_at, "finished_at": j.finished_at,
    })
}

async fn route(app: &AppHandle, req: &Req) -> Result<Value, String> {
    let st = app.state::<AppState>();
    let segs: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let body: Value = if req.body.is_empty() { json!({}) } else { serde_json::from_slice(&req.body).map_err(|x| format!("Invalid JSON: {x}"))? };
    match (req.method.as_str(), segs.as_slice()) {
        ("GET", ["v1", "status"]) => {
            let (wf, errs) = st.load_workflows();
            let jobs = st.db.jobs(None).map_err(|x| x.to_string())?;
            let active = jobs.iter().filter(|j| matches!(j.status.as_str(), "queued" | "routing" | "generating" | "downloading")).count();
            Ok(json!({
                "app_version": env!("CARGO_PKG_VERSION"),
                "secrets": secrets::status(),
                "endpoint": st.db.kv_get("endpoint_name").unwrap_or_default(),
                "default_gpu": st.db.kv_get("gpu_selection").unwrap_or_default(),
                "default_workflow": st.db.kv_get("last_workflow").unwrap_or_default(),
                "default_provider": st.db.kv_get("provider").unwrap_or_else(|| "vast".into()),
                "local_installed": crate::local::installed(&crate::local::portable_dir(&st)),
                "output_dir": st.output_dir().to_string_lossy(),
                "workflows": wf.len(), "workflow_errors": errs, "active_jobs": active,
            }))
        }
        ("GET", ["v1", "workflows"]) => {
            let (wf, _) = st.load_workflows();
            let list: Vec<Value> = wf
                .iter()
                .map(|w| {
                    let m = &w.meta;
                    let inputs: Vec<Value> = m
                        .inputs
                        .iter()
                        .map(|i| {
                            json!({
                                "key": i.key, "type": i.ty, "default": i.default, "min": i.min, "max": i.max, "required": i.required,
                                "label": if i.label_en.is_empty() { &i.key } else { &i.label_en },
                            })
                        })
                        .collect();
                    json!({ "id": m.id, "name": m.name_en, "description": m.desc_en, "license": if !m.license.is_empty() { m.license.clone() } else if m.provider.is_empty() { "open".into() } else { "paid".into() }, "good_for": m.good_en, "est_usd_per_result": m.est_usd, "kind": m.kind, "vram_gb": m.vram_gb, "est_seconds": m.est_sec, "pack": m.pack, "priority": m.priority, "inputs": inputs })
                })
                .collect();
            Ok(json!(list))
        }
        ("GET", ["v1", "gpus"]) => serde_json::to_value(gpu_offers(app.state(), query_f64(&req.query, "min_vram").unwrap_or(0.0)).await?).map_err(|x| x.to_string()),
        ("GET", ["v1", "projects"]) => serde_json::to_value(st.projects()?).map_err(|x| x.to_string()),
        ("GET", ["v1", "jobs"]) => {
            let mut jobs = st.db.jobs(None).map_err(|x| x.to_string())?;
            jobs.sort_by_key(|j| -j.created_at);
            Ok(json!(jobs.iter().take(50).map(job_json).collect::<Vec<_>>()))
        }
        ("GET", ["v1", "jobs", id]) => st.db.job(id).map(|j| job_json(&j)).ok_or_else(|| "Job not found".to_string()),
        ("POST", ["v1", "jobs"]) => create_job(app, &st, &body).await,
        ("POST", ["v1", "settings"]) => set_defaults(&st, &body).await,
        ("POST", ["v1", "jobs", id, "cancel"]) => {
            job_cancel(app.state(), app.clone(), id.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", ["v1", "jobs", id, "retry"]) => {
            job_retry(app.state(), app.clone(), id.to_string())?;
            Ok(json!({ "ok": true }))
        }
        ("POST", ["v1", "report"]) => Ok(json!({ "report": logs_report(app.state())? })),
        _ => Err(format!("Unknown route: {} {}", req.method, req.path)),
    }
}

async fn create_job(app: &AppHandle, st: &AppState, b: &Value) -> Result<Value, String> {
    let prompt = b["prompt"].as_str().unwrap_or("").trim().to_string();
    if prompt.is_empty() {
        return Err("The prompt is missing".into());
    }
    let mut params: Map<String, Value> = b["params"].as_object().cloned().unwrap_or_default();
    if let Some(p) = b["provider"].as_str().filter(|p| matches!(*p, "local" | "vast")) {
        params.insert("provider".into(), json!(p));
    }
    let mut gpu_note = Value::Null;
    if let Some(g) = b["gpu"].as_str().filter(|g| !g.is_empty()) {
        let (name, dph, vram) = resolve_gpu(st, g).await?;
        params.insert("gpu".into(), json!(name));
        params.insert("gpu_dph".into(), json!(dph));
        gpu_note = json!(format!("{name} ({vram} GB, ~{dph:.2} USD/hour)"));
    }
    if let Some(s) = b["scene"].as_u64() {
        params.insert("scene".into(), json!(s));
    }
    let has_start = match b["image_path"].as_str().filter(|p| !p.is_empty()) {
        Some(p) => {
            params.insert("image".into(), json!(crate::stash_input(st, std::path::Path::new(p), &uuid::Uuid::new_v4().simple().to_string())?));
            true
        }
        None => false,
    };
    let has_end = match b["end_image_path"].as_str().filter(|p| !p.is_empty()) {
        Some(p) => {
            params.insert("image_end".into(), json!(crate::stash_input(st, std::path::Path::new(p), &uuid::Uuid::new_v4().simple().to_string())?));
            true
        }
        None => false,
    };
    // chained / uploaded inputs: {"audio": "job:<id>"} (a finished result) or {"audio": "C:\path\voice.wav"} (a file on this PC)
    if let Some(map) = b["inputs"].as_object() {
        for (k, v) in map {
            let Some(v) = v.as_str().filter(|v| !v.is_empty()) else { continue };
            if !["image", "audio", "video"].iter().any(|p| k == p || k.starts_with(&format!("{p}_"))) {
                return Err(format!("\"{k}\" is not a media input (use image, audio or video keys)."));
            }
            let val = if v.starts_with("job:") { v.to_string() } else { crate::stash_input(st, std::path::Path::new(v), &uuid::Uuid::new_v4().simple().to_string())? };
            params.insert(k.clone(), json!(val));
        }
    }
    let (wfs, _) = st.load_workflows();
    let wanted_kind = if matches!(b["kind"].as_str(), Some("tts" | "vc" | "i2i" | "v2v" | "lipsync" | "subs" | "i2m")) { b["kind"].as_str().unwrap() }
        else if b["kind"] == "audio" { "t2a" } else if b["kind"] == "image" { "t2i" } else if has_end { "flf2v" } else if has_start { "i2v" } else { "t2v" };
    let wf_id = match b["workflow_id"].as_str().filter(|w| !w.is_empty()) {
        Some(q) => resolve_workflow(&wfs, q, wanted_kind)?,
        None => {
            let pick = |kind: &str| wfs.iter().filter(|w| w.meta.kind == kind).min_by_key(|w| w.meta.priority).map(|w| w.meta.id.clone());
            // the model family chosen last (in the app or by Claude) wins when it has a mode of the wanted kind
            let family = st.db.kv_get("default_model").unwrap_or_default();
            (if family.is_empty() { None } else { resolve_workflow(&wfs, &family, wanted_kind).ok() })
                .or_else(|| pick(wanted_kind))
                .or_else(|| if has_end { pick("i2v") } else { None })
                .ok_or_else(|| format!("There is no workflow of type {wanted_kind}"))?
        }
    };
    if (has_start || has_end) && !wfs.iter().any(|w| w.meta.id == wf_id && w.meta.inputs.iter().any(|i| i.key == "image")) {
        return Err(format!("The workflow \"{wf_id}\" does not take an image."));
    }
    let name = b["project"].as_str().map(str::trim).filter(|p| !p.is_empty()).unwrap_or("Claude");
    let existing = st.db.projects().map_err(|x| x.to_string())?.into_iter().find(|p| p.name.eq_ignore_ascii_case(name));
    let project_id = match existing {
        Some(p) => p.id,
        None => st.db.create_project(name, "").map_err(|x| x.to_string())?.id,
    };
    let job = job_enqueue(app.state(), app.clone(), project_id, wf_id, prompt, params)?;
    let mut out = job_json(&job);
    out["gpu_chosen"] = gpu_note;
    Ok(out)
}

fn norm(s: &str) -> String {
    let l = s.to_lowercase().replace("geforce", "").replace("nvidia", "").replace("rtx", "");
    l.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// "ltx", "wan", "ltxv-i2v" -> a workflow id, preferring the wanted kind (t2v | i2v | flf2v).
fn resolve_workflow(wfs: &[crate::workflows::Workflow], q: &str, kind: &str) -> Result<String, String> {
    let ql = q.trim().to_lowercase();
    if let Some(w) = wfs.iter().find(|w| w.meta.id == ql) {
        return Ok(w.meta.id.clone());
    }
    let key: String = ql.chars().filter(|c| c.is_alphanumeric()).collect();
    let mut hits: Vec<&crate::workflows::Workflow> = wfs
        .iter()
        .filter(|w| {
            let id: String = w.meta.id.chars().filter(|c| c.is_alphanumeric()).collect();
            id.contains(&key) || w.meta.name_en.to_lowercase().contains(&ql)
        })
        .collect();
    hits.sort_by_key(|w| (w.meta.kind != kind, w.meta.priority));
    hits.first().map(|w| w.meta.id.clone()).ok_or_else(|| {
        format!("Unknown model \"{q}\". Available: {}", wfs.iter().map(|w| w.meta.id.as_str()).collect::<Vec<_>>().join(", "))
    })
}

/// Splits "RTX 4090 24GB" into the card text ("rtx 4090") and the VRAM figure (24.0).
fn split_vram(q: &str) -> (String, Option<f64>) {
    let l = q.to_lowercase();
    let b = l.as_bytes();
    if let Some(i) = l.find("gb") {
        let mut j = i;
        while j > 0 && b[j - 1] == b' ' {
            j -= 1;
        }
        let end = j;
        while j > 0 && (b[j - 1].is_ascii_digit() || b[j - 1] == b'.') {
            j -= 1;
        }
        if let Ok(v) = l[j..end].parse::<f64>() {
            return (format!("{}{}", &l[..j], &l[i + 2..]), Some(v));
        }
    }
    (l, None)
}

/// Free text like "4090" or "RTX 4090 24GB" -> exact Vast GPU name, USD/h and VRAM. Never silently substitutes another card.
async fn resolve_gpu(st: &AppState, q: &str) -> Result<(String, f64, f64), String> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing in the app (Settings).")?;
    let offers = st.vast.gpu_offers(&key, 0.0).await?;
    let (text, want_vram) = split_vram(q);
    let core = norm(&text);
    let list = |o: &[&crate::vast::GpuOffer]| o.iter().map(|g| format!("{} ({} GB, {:.2} USD/hour)", g.name, g.vram_gb, g.min_price)).collect::<Vec<_>>().join("; ");
    let mut hits: Vec<&crate::vast::GpuOffer> = offers.iter().filter(|g| !core.is_empty() && norm(&g.name).contains(&core)).collect();
    if hits.is_empty() {
        let digits: String = core.chars().filter(|c| c.is_ascii_digit()).collect();
        let near: Vec<&crate::vast::GpuOffer> = offers.iter().filter(|g| !digits.is_empty() && norm(&g.name).contains(&digits)).collect();
        let all: Vec<&crate::vast::GpuOffer> = offers.iter().collect();
        return Err(format!(
            "The card \"{q}\" is not offered on Vast. {} Ask the user which one they want.",
            if near.is_empty() { format!("Available: {}", list(&all)) } else { format!("Close matches: {}", list(&near)) }
        ));
    }
    if let Some(v) = want_vram {
        let same: Vec<&crate::vast::GpuOffer> = hits.iter().copied().filter(|g| (g.vram_gb - v).abs() <= 1.5).collect();
        if same.is_empty() {
            return Err(format!("The card \"{q}\" exists with different VRAM: {}. Ask the user.", list(&hits)));
        }
        hits = same;
    }
    hits.sort_by_key(|g| norm(&g.name).len());
    let g = hits[0];
    Ok((g.name.clone(), g.median_price, g.vram_gb))
}

async fn set_defaults(st: &AppState, b: &Value) -> Result<Value, String> {
    let mut report = Map::new();
    if let Some(q) = b["gpu"].as_str() {
        if q.trim().is_empty() || q.eq_ignore_ascii_case("auto") {
            crate::apply_gpu_filter(st, &[], 0.0, 0.0, true).await?;
            report.insert("gpu".into(), json!("automatic choice"));
        } else {
            let (name, dph, vram) = resolve_gpu(st, q).await?;
            let msg = crate::apply_gpu_filter(st, &[name.clone()], vram, dph, true).await?;
            report.insert("gpu".into(), json!(format!("{name} ({vram} GB, ~{dph:.2} USD/hour)")));
            report.insert("note".into(), json!(msg.chars().take(160).collect::<String>()));
        }
    }
    if let Some(q) = b["model"].as_str().filter(|m| !m.trim().is_empty()) {
        let (wfs, _) = st.load_workflows();
        let id = resolve_workflow(&wfs, q, "t2v")?;
        let family: String = id.split('-').next().unwrap_or(&id).to_string();
        st.db.kv_set("default_model", &family).map_err(|x| x.to_string())?;
        st.db.kv_set("last_workflow", &id).map_err(|x| x.to_string())?;
        report.insert("model".into(), json!(family));
    }
    Ok(Value::Object(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vram_is_split_from_card_text() {
        assert_eq!(split_vram("RTX 4090 24GB"), ("rtx 4090 ".to_string(), Some(24.0)));
        assert_eq!(split_vram("4090TI 16 gb").1, Some(16.0));
        assert_eq!(split_vram("3090"), ("3090".to_string(), None));
    }

    #[test]
    fn card_names_are_normalised() {
        assert_eq!(norm("NVIDIA GeForce RTX 4090"), "4090");
        assert!(norm("RTX 3080 Ti").contains(&norm("3080ti")));
        assert!(!norm("RTX 4090").contains(&norm("4090ti")));
    }
}
