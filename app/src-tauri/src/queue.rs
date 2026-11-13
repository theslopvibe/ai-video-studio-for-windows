use crate::db::{now, Job};
use crate::{secrets, workflows, AppState};
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

#[derive(Clone, Copy, PartialEq)]
enum Class {
    /// this PC's ComfyUI: one at a time
    Single,
    /// hosted APIs (fal, Replicate, RunPod): they run on the provider's side
    Hosted,
}

/// "local" (this PC) or "runpod"; a per-video `provider` param wins over the setting. Anything but "local"
/// (including the old "vast" value of earlier versions) means RunPod.
pub fn job_provider(st: &AppState, job: &Job) -> String {
    let p: Map<String, Value> = serde_json::from_str(&job.params).unwrap_or_default();
    let v = p.get("provider").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from).or_else(|| st.db.kv_get("provider")).unwrap_or_else(|| "runpod".into());
    if v == "local" { v } else { "runpod".into() }
}

/// The mode that really runs: an open-source mode becomes its RunPod twin when the job runs on RunPod.
fn effective_id(st: &AppState, job: &Job) -> String {
    workflows::effective_id(&st.load_workflows().0, &job.workflow_id, &job_provider(st, job))
}

fn class_of(st: &AppState, job: &Job) -> Class {
    let id = effective_id(st, job);
    if st.load_workflows().0.into_iter().any(|w| w.meta.id == id && !w.meta.provider.is_empty()) { Class::Hosted } else { Class::Single }
}

/// A job that uses the result of an earlier job ("job:<id>") has to wait until that one is finished.
fn waits_for_earlier(st: &AppState, job: &Job) -> bool {
    let p: Map<String, Value> = serde_json::from_str(&job.params).unwrap_or_default();
    p.values().filter_map(|v| v.as_str()).filter_map(|v| v.strip_prefix("job:")).any(|id| st.db.job(id.trim()).is_some_and(|j| matches!(j.status.as_str(), "queued" | "routing" | "generating" | "downloading")))
}

/// Background loop: oldest job first; hosted jobs (RunPod, fal, ...) run side by side, local ones one at a time.
pub async fn run_loop(app: AppHandle) {
    let running: Arc<std::sync::Mutex<Vec<(String, Class)>>> = Default::default();
    loop {
        let st = app.state::<AppState>();
        let mut started = false;
        for j in st.db.queued_all() {
            let class = class_of(&st, &j);
            {
                let r = running.lock().unwrap();
                let n = r.iter().filter(|x| x.1 == class).count();
                let limit = match class { Class::Single => 1, Class::Hosted => 4 };
                if n >= limit || waits_for_earlier(&st, &j) {
                    continue;
                }
            }
            running.lock().unwrap().push((j.id.clone(), class));
            set(&app, &j.id, "routing", "Starting…", "");
            started = true;
            let (app2, running2) = (app.clone(), running.clone());
            tauri::async_runtime::spawn(async move {
                let flag = Arc::new(AtomicBool::new(false));
                app2.state::<AppState>().cancels.lock().unwrap().insert(j.id.clone(), flag.clone());
                crate::log::info(&format!(
                    "job {} start · mode={} · prompt={}",
                    j.id.chars().take(8).collect::<String>(), j.workflow_id, j.prompt.chars().take(100).collect::<String>()
                ));
                let res = run_job(&app2, &j, &flag).await;
                app2.state::<AppState>().cancels.lock().unwrap().remove(&j.id);
                let st = app2.state::<AppState>();
                let _ = st.db.set_job_times(&j.id, None, Some(now()));
                match res {
                    Ok(()) => {}
                    Err(e) if flag.load(Ordering::Relaxed) => {
                        let _ = e;
                        set(&app2, &j.id, "cancelled", "", "");
                    }
                    Err(e) => set(&app2, &j.id, "failed", "", &e),
                }
                emit(&app2, &j.id);
                running2.lock().unwrap().retain(|x| x.0 != j.id);
            });
        }
        if !started {
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
    }
}

async fn run_job(app: &AppHandle, job: &Job, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let st = app.state::<AppState>();
    // --- workflow
    let (wfs, _) = st.load_workflows();
    let wf_id = workflows::effective_id(&wfs, &job.workflow_id, &job_provider(&st, job));
    let wf = wfs
        .iter()
        .find(|w| w.meta.id == wf_id)
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
    // Hosted modes (RunPod, fal, Replicate, ...) run on the provider's side; every other mode runs on this PC.
    if !wf.meta.provider.is_empty() {
        return run_hosted(app, job, wf, &params, &workflow, cancel, t0).await;
    }
    run_local(app, job, &workflow, &params, cancel, wf.meta.name_en.clone(), t0).await
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

/// Generation through a hosted model API (RunPod, fal.ai, Replicate, ...).
async fn run_hosted(app: &AppHandle, job: &Job, wf: &workflows::Workflow, params: &Map<String, Value>, graph: &Value, cancel: &Arc<AtomicBool>, t0: Instant) -> Result<(), String> {
    let st = app.state::<AppState>();
    let m = &wf.meta;
    let secret = crate::hosted::secret_name(&m.provider).ok_or_else(|| format!("Unknown provider {}", m.provider))?;
    let key = secrets::get(secret).ok_or_else(|| format!("Missing {} key. Add it in Settings → Cloud providers.", m.provider))?;
    set(app, &job.id, "routing", &format!("Sending to {}…", m.provider), "");
    let (a, id, prov) = (app.clone(), job.id.clone(), m.provider.clone());
    let tick = move |s: u64| set(&a, &id, "generating", &format!("Generating on {prov}… {}", fmt_secs(s)), "");
    let mut cost = m.est_usd;
    let (bytes, ext) = if m.provider == "runpod" {
        // RunPod gets the filled ComfyUI graph itself, not a request template; the cost is the measured one.
        // The endpoint is woken first when it is asleep (the user never has to press anything).
        let endpoint = st.db.kv_get("runpod_endpoint_id").filter(|s| !s.trim().is_empty()).ok_or("Missing RunPod endpoint id. Add it in Settings → Cloud providers.")?;
        let dph = st.db.kv_get("runpod_gpu_dph").and_then(|s| s.trim().parse::<f64>().ok()).filter(|d| *d > 0.0).unwrap_or(crate::hosted::RUNPOD_DEFAULT_DPH);
        let (a2, id2) = (app.clone(), job.id.clone());
        let note = move |t: &str| set(&a2, &id2, "routing", t, "");
        let cap = crate::runpod::cap_usd(st.db.kv_get("runpod_cap_usd"));
        if crate::runpod::over_cap(crate::runpod::session_usd(), m.est_usd, cap) {
            return Err(format!("RunPod spend limit reached: ${:.2} spent in this session, the limit is ${cap:.2}. Raise it in Settings → Cloud providers (RunPod spend limit) if you want to continue.", crate::runpod::session_usd()));
        }
        crate::runpod::ensure_awake(&key, endpoint.trim(), &note).await;
        let (b, e, c) = crate::hosted::run_runpod(endpoint.trim(), &key, graph, dph, cancel, &tick, &note).await?;
        crate::runpod::add_spend(c);
        cost = c;
        (b, e)
    } else {
        let body = crate::hosted::render(&m.request, &workflows::hosted_params(wf, params));
        crate::hosted::run(&m.provider, &m.model, &key, &body, cancel, &tick).await?
    };
    set(app, &job.id, "downloading", "Saving…", "");
    let path = output_path(&st, job, params, &ext)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
    if st.db.projects().map_err(|e| e.to_string())?.into_iter().find(|p| p.id == job.project_id).map(|p| p.sidecar).unwrap_or(true) {
        let info = format!("{}\n\nmode: {} ({} {})\nparams: {}\ncreated: {}\n", job.prompt, m.name_en, m.provider, m.model, job.params, chrono::Local::now().to_rfc3339());
        let _ = tokio::fs::write(path.with_extension("txt"), info).await;
    }
    st.db.set_job_file(&job.id, &path.to_string_lossy()).ok();
    st.db.0.lock().unwrap().execute("UPDATE jobs SET est_cost=?2 WHERE id=?1", rusqlite::params![job.id, cost]).ok();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_slugged_and_timed() {
        assert_eq!(slug("A red fox, running!"), "A-red-fox-running");
        assert_eq!(slug("  "), "");
        assert_eq!(fmt_secs(125), "2:05");
    }
}
