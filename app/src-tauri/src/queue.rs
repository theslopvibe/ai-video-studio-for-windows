use crate::db::{now, Job};
use crate::{secrets, vast::Route, workflows, AppState};
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub fn emit(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    if let Some(j) = st.db.job(id) {
        let _ = app.emit("job-update", j);
    }
}

fn set(app: &AppHandle, id: &str, status: &str, message: &str, error: &str) {
    let st = app.state::<AppState>();
    let _ = st.db.update_job(id, status, message, error);
    let short: String = id.chars().take(8).collect();
    if !error.is_empty() {
        crate::log::error(&format!("job {short} {status}: {error}"));
    } else {
        crate::log::on_change(&format!("job-{id}"), "INFO", &format!("job {short} {status}: {message}"), status);
    }
    emit(app, id);
}

pub fn slug(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    t.chars().take(40).collect()
}

pub fn fmt_secs(s: u64) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

/// Background loop: one job at a time, oldest first.
pub async fn run_loop(app: AppHandle) {
    loop {
        let job = app.state::<AppState>().db.next_queued();
        match job {
            Some(j) => {
                let flag = Arc::new(AtomicBool::new(false));
                app.state::<AppState>().cancels.lock().unwrap().insert(j.id.clone(), flag.clone());
                crate::log::info(&format!(
                    "job {} start · mode={} · gpu={} · prompt={}",
                    j.id.chars().take(8).collect::<String>(), j.workflow_id, j.gpu, j.prompt.chars().take(100).collect::<String>()
                ));
                let res = run_job(&app, &j, &flag).await;
                app.state::<AppState>().cancels.lock().unwrap().remove(&j.id);
                let st = app.state::<AppState>();
                let _ = st.db.set_job_times(&j.id, None, Some(now()));
                match res {
                    Ok(()) => {}
                    Err(e) if flag.load(Ordering::Relaxed) => {
                        let _ = e;
                        set(&app, &j.id, "cancelled", "", "");
                    }
                    Err(e) => set(&app, &j.id, "failed", "", &e),
                }
                emit(&app, &j.id);
            }
            None => tokio::time::sleep(Duration::from_millis(700)).await,
        }
    }
}

async fn cancelled_wait(flag: &AtomicBool) {
    while !flag.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

async fn run_job(app: &AppHandle, job: &Job, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let st = app.state::<AppState>();
    let hosted = st.load_workflows().0.into_iter().find(|w| w.meta.id == job.workflow_id && !w.meta.provider.is_empty());
    let local = hosted.is_none() && job_provider(&st, job) == "local";
    let key = if local || hosted.is_some() { String::new() } else { secrets::get("vast_api_key").ok_or("Missing Vast API key (Settings).")? };
    let endpoint = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());

    // --- workflow
    let (wfs, _) = st.load_workflows();
    let wf = wfs
        .iter()
        .find(|w| w.meta.id == job.workflow_id)
        .ok_or_else(|| format!("Unknown workflow: {}", job.workflow_id))?;
    let mut params: Map<String, Value> = serde_json::from_str(&job.params).unwrap_or_default();
    // a step chained behind an earlier job ("job:<id>") picks up that job's finished file now
    crate::resolve_media_refs(&st, &job.id, &mut params, true)?;
    for def in wf.meta.inputs.iter().filter(|i| matches!(i.ty.as_str(), "image" | "audio" | "video")) {
        if let Some(path) = params.get(&def.key).and_then(|v| v.as_str()).and_then(|s| s.strip_prefix("file:")).map(String::from) {
            use base64::Engine;
            if !crate::is_input_file(&st, &path) {
                return Err("Image path is outside the app's inputs folder.".into());
            }
            let bytes = tokio::fs::read(&path).await.map_err(|e| format!("Image not found ({path}): {e}"))?;
            params.insert(def.key.clone(), json!(base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
        if def.required && params.get(&def.key).and_then(|v| v.as_str()).unwrap_or("").is_empty() {
            return Err(format!("This mode needs a {}: \"{}\". Pick it on the Create page.", def.ty, if def.label_en.is_empty() { &def.key } else { &def.label_en }));
        }
    }
    let workflow = workflows::fill(wf, &params);

    let t0 = Instant::now();
    st.db.set_job_times(&job.id, Some(now()), None).ok();
    if let Some(h) = &hosted {
        return run_hosted(app, job, h, &params, cancel, t0).await;
    }
    if local {
        return run_local(app, job, &workflow, &params, cancel, wf.meta.name_en.clone(), t0).await;
    }

    if st.db.kv_get("vast_mode").as_deref() != Some("serverless") {
        let pack = if wf.meta.pack.is_empty() { vec![] } else { vec![wf.meta.pack.clone()] };
        return run_card(app, job, &workflow, &params, &pack, cancel, wf.meta.name_en.clone(), t0).await;
    }

    // Card choice: this video's own pick, else the global selection. Only touch Vast when it changed.
    let own_gpu = params.get("gpu").and_then(|v| v.as_str()).filter(|g| !g.is_empty()).map(String::from);
    let want: Vec<String> = match &own_gpu {
        Some(g) => vec![g.clone()],
        None => st.db.kv_get("gpu_selection").unwrap_or_default().split(',').filter(|x| !x.is_empty()).map(String::from).collect(),
    };
    if !want.is_empty() && st.db.kv_get("gpu_applied").unwrap_or_default() != want.join(",") {
        set(app, &job.id, "routing", &format!("Setting the card: {}", want.join(", ")), "");
        let dph = params.get("gpu_dph").and_then(|v| v.as_f64()).unwrap_or(0.0);
        crate::apply_gpu_filter(&st, &want, wf.meta.vram_gb as f64, dph, false).await?;
    }

    // --- 1. route (cold start can take minutes)
    set(app, &job.id, "routing", "Looking for a worker…", "");
    let mut idx: i64 = 0;
    let mut attempt = 0u32;
    let auth = loop {
        if cancel.load(Ordering::Relaxed) { return Err("cancelled".into()); }
        if t0.elapsed() > Duration::from_secs(20 * 60) {
            return Err("No worker started within 20 minutes. Check the endpoint and GPU availability on Vast.".into());
        }
        match st.vast.route(&key, &endpoint, idx).await? {
            Route::Ready { body, .. } => break body,
            Route::Waiting { request_idx, status } => {
                idx = request_idx;
                let label = if status.is_empty() { "Starting a worker".to_string() } else { format!("Starting a worker ({status})") };
                set(app, &job.id, "routing", &format!("{label} · {}", fmt_secs(t0.elapsed().as_secs())), "");
                attempt += 1;
                let wait = (1.5f32 * attempt as f32).min(6.0);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs_f32(wait)) => {}
                    _ = cancelled_wait(cancel) => return Err("cancelled".into()),
                }
            }
        }
    };

    // --- 2. generate (blocking call; tick the clock meanwhile)
    let gen_start = Instant::now();
    let mut local = st.db.kv_get("delivery").map(|d| d != "cloud").unwrap_or(true);
    let (resp, local) = loop {
        let wf = prepare(&workflow, local);
        let msg = if local { "Generating video…" } else { "Generating video (via R2)…" };
        set(app, &job.id, "generating", msg, "");
        let fut = st.vast.generate(&key, &auth, &wf, &job.id);
        tokio::pin!(fut);
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        let resp = loop {
            tokio::select! {
                r = &mut fut => break r?,
                _ = tick.tick() => set(app, &job.id, "generating", &format!("{msg} {}", fmt_secs(gen_start.elapsed().as_secs())), ""),
                _ = cancelled_wait(cancel) => return Err("cancelled".into()),
            }
        };
        if local && node_missing(&resp) {
            // The worker has no inline-save node yet: fall back to the S3/R2 route once.
            local = false;
            continue;
        }
        break (resp, local);
    };

    let status = resp["status"].as_str().unwrap_or("");
    // --- 3. deliver: inline bytes (local mode) or a pre-signed URL
    set(app, &job.id, "downloading", "Saving…", "");
    let project = st.db.projects().map_err(|e| e.to_string())?.into_iter().find(|p| p.id == job.project_id);
    let sidecar = project.as_ref().map(|p| p.sidecar).unwrap_or(true);
    let dir = match &project {
        Some(p) => st.project_dir(p),
        None => st.output_dir().join("Default"),
    };
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let scene = params.get("scene").and_then(|v| v.as_u64()).filter(|n| *n > 0).map(|n| format!("s{n:02}_")).unwrap_or_default();
    let path = dir.join(format!("{stamp}_{scene}{}.mp4", slug(&job.prompt)));

    if let Some(b64) = find_inline(&resp["comfyui_response"]) {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|e| format!("base64: {e}"))?;
        tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
    } else {
        let url = resp["output"]
            .as_array()
            .and_then(|a| a.iter().find_map(|o| o["url"].as_str().filter(|u| !u.is_empty())))
            .map(String::from);
        let Some(url) = url else {
            let msg = resp["message"].as_str().unwrap_or("");
            let detail = resp["comfyui_response"]["execution_details"]["error"].to_string();
            let made = resp["output"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
            return Err(if made {
                "The video was made on the worker but came back neither inline nor as a URL. Install the local-save node (Models → Install on workers) or set S3/R2 in the Vast environment variables.".into()
            } else {
                format!("Generation failed (status={status}) {msg} {}", if detail == "null" { "" } else { &detail })
            });
        };
        download(app, &job.id, &url, &path, cancel).await?;
    }
    let _ = local;

    if sidecar {
        let wf_name = wfs.iter().find(|w| w.meta.id == job.workflow_id).map(|w| w.meta.name_en.clone()).unwrap_or_default();
        let info = format!("{}\n\nworkflow: {wf_name}\nparams: {}\ncreated: {}\n", job.prompt, job.params, chrono::Local::now().to_rfc3339());
        let _ = tokio::fs::write(path.with_extension("txt"), info).await;
    }
    st.db.set_job_file(&job.id, &path.to_string_lossy()).ok();
    let secs = t0.elapsed().as_secs_f64();
    let dph: f64 = params.get("gpu_dph").and_then(|v| v.as_f64()).filter(|d| *d > 0.0).or_else(|| st.db.kv_get("dph_estimate").and_then(|s| s.parse().ok())).unwrap_or(0.0);
    let cost = dph * secs / 3600.0;
    st.db.0.lock().unwrap().execute("UPDATE jobs SET est_cost=?2 WHERE id=?1", rusqlite::params![job.id, cost]).ok();
    set(app, &job.id, "done", &format!("Done in {}", fmt_secs(secs as u64)), "");
    let _ = json!(null);
    Ok(())
}

/// "local" (this PC) or "vast"; a per-video `provider` param wins over the setting.
fn job_provider(st: &AppState, job: &Job) -> String {
    let p: Map<String, Value> = serde_json::from_str(&job.params).unwrap_or_default();
    p.get("provider").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from).or_else(|| st.db.kv_get("provider")).unwrap_or_else(|| "vast".into())
}

fn output_path(st: &AppState, job: &Job, params: &Map<String, Value>, ext: &str) -> Result<std::path::PathBuf, String> {
    let project = st.db.projects().map_err(|e| e.to_string())?.into_iter().find(|p| p.id == job.project_id);
    let dir = match &project {
        Some(p) => st.project_dir(p),
        None => st.output_dir().join("Default"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let scene = params.get("scene").and_then(|v| v.as_u64()).filter(|n| *n > 0).map(|n| format!("s{n:02}_")).unwrap_or_default();
    Ok(dir.join(format!("{stamp}_{scene}{}.{ext}", slug(&job.prompt))))
}

/// Generation through a hosted model API (fal.ai, Replicate).
async fn run_hosted(app: &AppHandle, job: &Job, wf: &workflows::Workflow, params: &Map<String, Value>, cancel: &Arc<AtomicBool>, t0: Instant) -> Result<(), String> {
    let st = app.state::<AppState>();
    let m = &wf.meta;
    let secret = crate::hosted::secret_name(&m.provider).ok_or_else(|| format!("Unknown provider {}", m.provider))?;
    let key = secrets::get(secret).ok_or_else(|| format!("Missing {} key. Add it in Settings → Cloud providers.", m.provider))?;
    let body = crate::hosted::render(&m.request, &workflows::hosted_params(wf, params));
    set(app, &job.id, "routing", &format!("Sending to {}…", m.provider), "");
    let (a, id, prov) = (app.clone(), job.id.clone(), m.provider.clone());
    let tick = move |s: u64| set(&a, &id, "generating", &format!("Generating on {prov}… {}", fmt_secs(s)), "");
    let (bytes, ext) = crate::hosted::run(&m.provider, &m.model, &key, &body, cancel, &tick).await?;
    set(app, &job.id, "downloading", "Saving…", "");
    let path = output_path(&st, job, params, &ext)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
    if st.db.projects().map_err(|e| e.to_string())?.into_iter().find(|p| p.id == job.project_id).map(|p| p.sidecar).unwrap_or(true) {
        let info = format!("{}\n\nmode: {} ({} {})\nparams: {}\ncreated: {}\n", job.prompt, m.name_en, m.provider, m.model, job.params, chrono::Local::now().to_rfc3339());
        let _ = tokio::fs::write(path.with_extension("txt"), info).await;
    }
    st.db.set_job_file(&job.id, &path.to_string_lossy()).ok();
    st.db.0.lock().unwrap().execute("UPDATE jobs SET est_cost=?2 WHERE id=?1", rusqlite::params![job.id, m.est_usd]).ok();
    set(app, &job.id, "done", &format!("Done in {}", fmt_secs(t0.elapsed().as_secs())), "");
    Ok(())
}

/// Generation on this PC through the local ComfyUI server.
async fn run_local(app: &AppHandle, job: &Job, workflow: &Value, params: &Map<String, Value>, cancel: &Arc<AtomicBool>, wf_name: String, t0: Instant) -> Result<(), String> {
    let st = app.state::<AppState>();
    set(app, &job.id, "routing", "Starting ComfyUI on this PC…", "");
    let (a, id) = (app.clone(), job.id.clone());
    let tick = move |s: u64| set(&a, &id, "generating", &format!("Generating on this PC… {}", fmt_secs(s)), "");
    let (bytes, ext) = crate::local::generate(&st, workflow, cancel, &tick).await?;
    save_result(app, job, params, &wf_name, t0, bytes, ext).await
}

/// Rented Vast card: starts one if none is ready (this takes minutes the first time), then runs the workflow on it.
async fn run_card(app: &AppHandle, job: &Job, workflow: &Value, params: &Map<String, Value>, packs: &[String], cancel: &Arc<AtomicBool>, wf_name: String, t0: Instant) -> Result<(), String> {
    let st = app.state::<AppState>();
    let gpu = params.get("gpu").and_then(|v| v.as_str()).filter(|g| !g.is_empty()).map(String::from)
        .or_else(|| st.db.kv_get("gpu_selection").and_then(|g| g.split(',').find(|x| !x.is_empty()).map(String::from)))
        .unwrap_or_else(|| "RTX 4090".into());
    let (a, id) = (app.clone(), job.id.clone());
    let say = move |m: &str| set(&a, &id, "routing", m, "");
    let card = crate::card::ensure(&st, &gpu, packs, cancel, &say).await?;
    {
        use tauri::Emitter;
        let _ = app.emit("warmup", serde_json::json!({ "state": "ready", "text": "", "secs": 0 }));
    }
    let (a, id) = (app.clone(), job.id.clone());
    let tick = move |s: u64| set(&a, &id, "generating", &format!("Generating on the rented card… {}", fmt_secs(s)), "");
    let mut wf = workflow.clone();
    crate::local::randomize_seeds(&mut wf);
    let (bytes, ext) = crate::local::generate_at(&card.client, &card.base, Some(&card.token), &wf, cancel, &tick).await?;
    save_result(app, job, params, &wf_name, t0, bytes, ext).await
}

async fn save_result(app: &AppHandle, job: &Job, params: &Map<String, Value>, wf_name: &str, t0: Instant, bytes: Vec<u8>, ext: String) -> Result<(), String> {
    let st = app.state::<AppState>();
    set(app, &job.id, "downloading", "Saving…", "");
    let path = output_path(&st, job, params, &ext)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
    if st.db.projects().map_err(|e| e.to_string())?.into_iter().find(|p| p.id == job.project_id).map(|p| p.sidecar).unwrap_or(true) {
        let info = format!("{}

workflow: {wf_name}
params: {}
created: {}
", job.prompt, job.params, chrono::Local::now().to_rfc3339());
        let _ = tokio::fs::write(path.with_extension("txt"), info).await;
    }
    st.db.set_job_file(&job.id, &path.to_string_lossy()).ok();
    set(app, &job.id, "done", &format!("Done in {}", fmt_secs(t0.elapsed().as_secs())), "");
    Ok(())
}

async fn download(app: &AppHandle, id: &str, url: &str, path: &std::path::Path, cancel: &AtomicBool) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let r = reqwest::Client::new().get(url).timeout(Duration::from_secs(600)).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("Download failed: HTTP {}", r.status()));
    }
    let total = r.content_length().unwrap_or(0);
    let tmp = path.with_extension("part");
    let mut f = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
    let mut got = 0u64;
    let mut s = r.bytes_stream();
    let mut last = Instant::now();
    while let Some(chunk) = s.next().await {
        if cancel.load(Ordering::Relaxed) {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err("cancelled".into());
        }
        let c = chunk.map_err(|e| e.to_string())?;
        got += c.len() as u64;
        f.write_all(&c).await.map_err(|e| e.to_string())?;
        if last.elapsed() > Duration::from_millis(400) {
            last = Instant::now();
            let m = if total > 0 { format!("Downloading… {}%", got * 100 / total) } else { format!("Downloading… {} MB", got / 1_000_000) };
            set(app, id, "downloading", &m, "");
        }
    }
    f.flush().await.map_err(|e| e.to_string())?;
    drop(f);
    tokio::fs::rename(&tmp, path).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Local mode: swap the S3-uploading SaveVideo for the inline-save node.
fn prepare(wf: &Value, local: bool) -> Value {
    let mut w = wf.clone();
    if !local {
        return w;
    }
    let Some(obj) = w.as_object_mut() else { return w };
    let create = obj.iter().find(|(_, n)| n["class_type"] == "CreateVideo").map(|(k, _)| k.clone());
    let save = obj.iter().find(|(_, n)| n["class_type"] == "SaveVideo").map(|(k, _)| k.clone());
    if let Some(c) = create {
        obj.insert(
            "9999".into(),
            json!({"class_type": "AIVSSaveVideoBase64", "inputs": {"video": [c, 0]}, "_meta": {"title": "AIVS inline save"}}),
        );
        if let Some(s) = save {
            obj.remove(&s);
        }
    }
    w
}

fn node_missing(resp: &Value) -> bool {
    if resp["status"] == "completed" {
        return false;
    }
    let t = resp.to_string().to_lowercase();
    t.contains("aivssavevideobase64") && (t.contains("missing_node_type") || t.contains("does not exist") || t.contains("not found") || t.contains("unknown"))
}

fn find_inline(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => {
            if let Some(Value::Array(a)) = m.get("aivs_video_b64") {
                if let Some(Value::String(s)) = a.first() {
                    return Some(s.clone());
                }
            }
            m.values().find_map(find_inline)
        }
        Value::Array(a) => a.iter().find_map(find_inline),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_mode_swaps_save_node_and_detects_inline() {
        let wf = json!({"98": {"class_type":"SaveVideo","inputs":{"video":["100",0]}}, "100": {"class_type":"CreateVideo","inputs":{}}});
        let l = prepare(&wf, true);
        assert!(l.get("98").is_none());
        assert_eq!(l["9999"]["inputs"]["video"], json!(["100", 0]));
        assert!(prepare(&wf, false).get("98").is_some());
        let r = json!({"x": {"outputs": {"9999": {"aivs_video_b64": ["QUJD"]}}}});
        assert_eq!(find_inline(&r).as_deref(), Some("QUJD"));
        assert!(node_missing(&json!({"status":"failed","message":"missing_node_type AIVSSaveVideoBase64"})));
        assert!(!node_missing(&json!({"status":"completed"})));
    }
}
