mod anthropic;
mod card;
mod api;
mod chat;
mod hf;
mod hosted;
mod log;
mod db;
mod editor;
mod llm;
mod local;
mod mcp;
mod provision;
mod queue;
mod secrets;
mod vast;
mod workflows;

use db::{Db, Job, Project};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

pub struct AppState {
    pub db: Db,
    pub vast: vast::Vast,
    pub cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub local: local::Local,
    pub data_dir: PathBuf,
    pub bundled_dir: PathBuf,
    pub videos_dir: PathBuf,
}

impl AppState {
    pub fn load_workflows(&self) -> (Vec<workflows::Workflow>, Vec<String>) {
        workflows::load_all(&self.bundled_dir, &workflows::user_dir(&self.data_dir))
    }
    /// The user's folder for a project, or <output folder>/<project name>.
    pub fn project_dir(&self, p: &Project) -> PathBuf {
        if p.folder.trim().is_empty() {
            self.output_dir().join(queue::slug(&p.name))
        } else {
            PathBuf::from(p.folder.trim())
        }
    }
    pub fn projects(&self) -> Result<Vec<Project>, String> {
        let mut v = self.db.projects().map_err(|x| x.to_string())?;
        for p in v.iter_mut() {
            p.dir = self.project_dir(p).to_string_lossy().into();
        }
        Ok(v)
    }
    /// The webview may read pictures/clips only from the folders the app itself works in (output, projects, app data).
    pub fn sync_asset_scope(&self, app: &AppHandle) {
        let scope = app.asset_protocol_scope();
        let mut dirs = vec![self.output_dir(), self.data_dir.clone(), self.videos_dir.clone()];
        dirs.extend(self.projects().unwrap_or_default().into_iter().map(|p| PathBuf::from(p.dir)));
        for d in dirs {
            let _ = std::fs::create_dir_all(&d);
            let _ = scope.allow_directory(&d, true);
        }
    }
    pub fn output_dir(&self) -> PathBuf {
        match self.db.kv_get("out_dir") {
            Some(d) if !d.trim().is_empty() => PathBuf::from(d),
            _ => self.videos_dir.clone(),
        }
    }
}

type R<T> = Result<T, String>;
fn e(x: impl std::fmt::Display) -> String {
    x.to_string()
}

/// Bundled catalog (models.json) plus the user's custom packs added from HuggingFace.
pub fn catalog(st: &AppState) -> Vec<Value> {
    let mut cat: Vec<Value> = serde_json::from_str(include_str!("../models.json")).unwrap_or_default();
    let custom: Vec<Value> = st.db.kv_get("custom_models").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    cat.extend(custom);
    cat
}

const SETTING_KEYS: &[&str] = &[
    "lang", "endpoint_name", "out_dir", "anthropic_model", "dph_estimate", "gpu_selection", "last_project",
    "last_workflow", "models_selected", "enhance_provider", "claude_model", "delivery", "worker_pack", "gemini_model", "theme", "openai_base_url", "openai_model", "provider", "local_comfy_dir", "setup_done", "api_enabled", "auto_close_min", "card_models",
];

#[derive(Serialize)]
struct Bootstrap {
    projects: Vec<Project>,
    secrets: BTreeMap<String, bool>,
    settings: BTreeMap<String, String>,
    workflows: Vec<workflows::Workflow>,
    workflow_errors: Vec<String>,
    user_workflows_dir: String,
    output_dir: String,
    anthropic_models: Vec<&'static str>,
    cli_models: Vec<&'static str>,
    claude_cli: Option<String>,
    models: Value,
}

#[tauri::command]
fn bootstrap(st: State<AppState>) -> R<Bootstrap> {
    let (wf, errs) = st.load_workflows();
    let dir = workflows::user_dir(&st.data_dir);
    let _ = std::fs::create_dir_all(&dir);
    let settings = SETTING_KEYS
        .iter()
        .filter_map(|k| st.db.kv_get(k).map(|v| (k.to_string(), v)))
        .collect();
    Ok(Bootstrap {
        projects: st.projects()?,
        secrets: secrets::status(),
        settings,
        workflows: wf,
        workflow_errors: errs,
        user_workflows_dir: dir.to_string_lossy().into(),
        output_dir: st.output_dir().to_string_lossy().into(),
        anthropic_models: anthropic::MODELS.to_vec(),
        cli_models: anthropic::CLI_MODELS.to_vec(),
        claude_cli: anthropic::locate_cli().map(|p| p.to_string_lossy().into()),
        models: Value::Array(catalog(&st)),
    })
}

#[tauri::command]
fn reload_workflows(st: State<AppState>) -> (Vec<workflows::Workflow>, Vec<String>) {
    st.load_workflows()
}

/// Only files inside the app's own `inputs` folder may be read or deleted through a `file:` job parameter.
const MEDIA_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "mp3", "wav", "flac", "ogg", "opus", "m4a", "mp4", "webm", "mov", "mkv"];

/// `image`, `audio_target`, `video_2`: a media type, optionally followed by `_` and lowercase letters/digits only.
fn is_media_key(k: &str) -> bool {
    ["image", "audio", "video"].iter().any(|p| {
        k == *p || k.strip_prefix(p).and_then(|r| r.strip_prefix('_')).is_some_and(|r| !r.is_empty() && r.len() <= 24 && r.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
    })
}

/// Only `X:\...` (plain or verbatim drive) passes: no UNC shares, device paths or relative paths.
fn is_local_drive_path(p: &Path) -> bool {
    use std::path::{Component, Prefix};
    let mut c = p.components();
    match c.next() {
        Some(Component::Prefix(x)) => match x.kind() {
            Prefix::VerbatimDisk(_) => true,
            Prefix::Disk(_) => matches!(c.next(), Some(Component::RootDir)),
            _ => false,
        },
        _ => false,
    }
}

/// Copies a picture, sound or clip into the inputs folder and returns the `file:` reference jobs use.
/// `name==1.2.3` only: letters, digits, `-`, `_`, `.`, no spaces, no options, no URLs.
pub fn safe_pip_spec(s: &str) -> bool {
    let Some((name, ver)) = s.split_once("==") else { return false };
    !name.is_empty() && name.len() <= 60 && !ver.is_empty() && ver.len() <= 24
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && ver.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
        && !name.starts_with('-')
}

pub fn stash_input(st: &AppState, src: &Path, tag: &str) -> Result<String, String> {
    // decided on the text alone, before any filesystem call
    if !is_local_drive_path(src) {
        return Err("Only files on a local drive (like C:\\...) are allowed; network and device paths are not.".into());
    }
    if std::fs::symlink_metadata(src).map_err(e)?.file_type().is_symlink() {
        return Err("Links are not followed; pick the file itself.".into());
    }
    let real = std::fs::canonicalize(src).map_err(e)?;
    if real.to_string_lossy().starts_with("\\\\?\\UNC") {
        return Err("Network paths are not allowed.".into());
    }
    let src = real.as_path();
    if !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("Bad file tag.".into());
    }
    let ext = src.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if !MEDIA_EXT.contains(&ext.as_str()) {
        return Err(format!("Unsupported file type: .{ext}"));
    }
    let meta = std::fs::metadata(src).map_err(e)?;
    if !meta.is_file() || meta.len() > 600 << 20 {
        return Err("The file is missing or too large (limit 600 MB).".into());
    }
    let dir = st.data_dir.join("inputs");
    std::fs::create_dir_all(&dir).map_err(e)?;
    let dest = dir.join(format!("{tag}.{ext}"));
    if dest.parent() != Some(dir.as_path()) {
        return Err("Bad destination.".into());
    }
    std::fs::copy(src, &dest).map_err(e)?;
    Ok(format!("file:{}", dest.to_string_lossy()))
}

/// `job:<id>` in an image/audio/video parameter means "the result of that earlier job": this is how results are chained
/// (picture → video → voice → lip-sync) from the UI, the chat and the MCP tools without anyone handling file paths.
pub fn resolve_media_refs(st: &AppState, job_id: &str, p: &mut Map<String, Value>, run_now: bool) -> R<()> {
    let keys: Vec<String> = p.keys().filter(|k| is_media_key(k)).cloned().collect();
    for k in keys {
        let Some(Value::String(v)) = p.get(&k).cloned() else { continue };
        if let Some(id) = v.strip_prefix("job:") {
            let job = st.db.jobs(None).map_err(e)?.into_iter().find(|j| j.id == id.trim()).ok_or("The earlier result was not found.")?;
            if job.status != "done" || job.file_path.is_empty() {
                // queued behind it: the reference is resolved when this job starts (the queue runs jobs in order)
                if !run_now && matches!(job.status.as_str(), "queued" | "routing" | "generating" | "downloading") {
                    continue;
                }
                return Err("That earlier result is not available (it failed, was cancelled or is not finished).".into());
            }
            p.insert(k.clone(), json!(stash_input(st, Path::new(&job.file_path), &format!("{job_id}-{}", uuid::Uuid::new_v4().simple()))?));
        } else if let Some(path) = v.strip_prefix("file:") {
            if !is_input_file(st, path) {
                return Err("File reference outside the inputs folder.".into());
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn input_import(st: State<AppState>, path: String) -> R<String> {
    stash_input(&st, Path::new(&path), &uuid::Uuid::new_v4().simple().to_string())
}

pub fn is_input_file(st: &AppState, path: &str) -> bool {
    match (std::fs::canonicalize(path), std::fs::canonicalize(st.data_dir.join("inputs"))) {
        (Ok(p), Ok(dir)) => p.starts_with(dir),
        _ => false,
    }
}

#[tauri::command]
fn setting_set(st: State<AppState>, app: AppHandle, key: String, value: String) -> R<()> {
    if !SETTING_KEYS.contains(&key.as_str()) {
        return Err(format!("unknown setting {key}"));
    }
    st.db.kv_set(&key, &value).map_err(e)?;
    if key == "out_dir" {
        st.sync_asset_scope(&app);
    }
    if key == "api_enabled" {
        api::set_enabled(&app, value == "1");
    }
    Ok(())
}

#[tauri::command]
fn secret_set(name: String, value: String) -> R<BTreeMap<String, bool>> {
    secrets::set(&name, &value).map_err(e)?;
    Ok(secrets::status())
}

// ---- projects

#[tauri::command]
fn project_create(st: State<AppState>, app: AppHandle, name: String, folder: String) -> R<Vec<Project>> {
    if name.trim().is_empty() {
        return Err("The name is empty".into());
    }
    if !folder.trim().is_empty() {
        std::fs::create_dir_all(folder.trim()).map_err(|x| format!("The folder cannot be created: {x}"))?;
    }
    st.db.create_project(&name, &folder).map_err(e)?;
    st.sync_asset_scope(&app);
    st.projects()
}

#[tauri::command]
fn project_update(st: State<AppState>, app: AppHandle, id: String, name: String, folder: String, suffix: String, sidecar: bool) -> R<Vec<Project>> {
    if name.trim().is_empty() {
        return Err("The name is empty".into());
    }
    if !folder.trim().is_empty() {
        std::fs::create_dir_all(folder.trim()).map_err(|x| format!("The folder cannot be created: {x}"))?;
    }
    st.db.update_project(&id, &name, &folder, &suffix, sidecar).map_err(e)?;
    st.sync_asset_scope(&app);
    st.projects()
}

#[tauri::command]
fn project_delete(st: State<AppState>, id: String) -> R<Vec<Project>> {
    if st.db.projects().map_err(e)?.len() <= 1 {
        return Err("There must be at least one project".into());
    }
    st.db.delete_project(&id).map_err(e)?;
    st.projects()
}

// ---- jobs

#[tauri::command]
fn jobs_list(st: State<AppState>) -> R<Vec<Job>> {
    st.db.jobs(None).map_err(e)
}

#[tauri::command]
fn job_enqueue(
    st: State<AppState>,
    app: AppHandle,
    project_id: String,
    workflow_id: String,
    prompt: String,
    params: Map<String, Value>,
) -> R<Job> {
    if prompt.trim().is_empty() {
        return Err("The prompt is empty".into());
    }
    let mut p = params;
    let job_id = uuid::Uuid::new_v4().to_string();
    // Start/end images (base64 JPEG from the UI) are kept as files so the jobs table stays small and retry works.
    resolve_media_refs(&st, &job_id, &mut p, false)?;
    let image_keys: Vec<String> = p.keys().filter(|k| *k == "image" || k.starts_with("image_")).cloned().collect();
    for k in image_keys {
        let Some(Value::String(b64)) = p.get(&k).cloned() else { continue };
        if b64.is_empty() {
            p.remove(&k);
            continue;
        }
        if b64.starts_with("file:") || b64.starts_with("job:") {
            continue;
        }
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|x| format!("Invalid image: {x}"))?;
        let dir = st.data_dir.join("inputs");
        std::fs::create_dir_all(&dir).map_err(e)?;
        let path = dir.join(format!("{job_id}-{k}.jpg"));
        std::fs::write(&path, bytes).map_err(e)?;
        p.insert(k, json!(format!("file:{}", path.to_string_lossy())));
    }
    let suffix = st.db.projects().map_err(e)?.into_iter().find(|x| x.id == project_id).map(|x| x.suffix).unwrap_or_default();
    let prompt = if suffix.trim().is_empty() { prompt.trim().to_string() } else { format!("{}, {}", prompt.trim().trim_end_matches(','), suffix.trim()) };
    p.insert("prompt".into(), json!(prompt));
    let gpu = p.get("gpu").and_then(|v| v.as_str()).filter(|g| !g.is_empty()).map(String::from).unwrap_or_else(|| st.db.kv_get("gpu_selection").unwrap_or_default());
    let j = Job {
        id: job_id,
        project_id,
        workflow_id,
        prompt: prompt.clone(),
        params: Value::Object(p).to_string(),
        status: "queued".into(),
        message: "Queued".into(),
        error: String::new(),
        file_path: String::new(),
        created_at: db::now(),
        started_at: 0,
        finished_at: 0,
        est_cost: 0.0,
        gpu,
    };
    st.db.insert_job(&j).map_err(e)?;
    queue::emit(&app, &j.id);
    Ok(j)
}

#[tauri::command]
fn job_cancel(st: State<AppState>, app: AppHandle, id: String) -> R<()> {
    if let Some(f) = st.cancels.lock().unwrap().get(&id) {
        f.store(true, Ordering::Relaxed);
        return Ok(());
    }
    if let Some(j) = st.db.job(&id) {
        if j.status == "queued" {
            st.db.update_job(&id, "cancelled", "", "").map_err(e)?;
            queue::emit(&app, &id);
        }
    }
    Ok(())
}

#[tauri::command]
fn job_retry(st: State<AppState>, app: AppHandle, id: String) -> R<()> {
    st.db.requeue_job(&id).map_err(e)?;
    queue::emit(&app, &id);
    Ok(())
}

#[tauri::command]
fn job_delete(st: State<AppState>, id: String, delete_file: bool) -> R<()> {
    if let Some(j) = st.db.job(&id) {
        if delete_file && !j.file_path.is_empty() {
            let _ = std::fs::remove_file(&j.file_path);
        }
        // start/end images of this job live in the app data folder: remove them with the job
        if let Ok(Value::Object(p)) = serde_json::from_str::<Value>(&j.params) {
            for v in p.values().filter_map(|v| v.as_str()).filter_map(|s| s.strip_prefix("file:")) {
                if is_input_file(&st, v) {
                    let _ = std::fs::remove_file(v);
                }
            }
        }
        log::info(&format!("job {} deleted (file too: {delete_file})", id.chars().take(8).collect::<String>()));
    }
    st.db.delete_job(&id).map_err(e)
}

// ---- prompt enhance

#[derive(Serialize)]
struct Enhanced {
    text: String,
    tokens: Option<u64>,
    provider: String,
}

fn prov(st: &AppState) -> String {
    match st.db.kv_get("enhance_provider").as_deref() {
        Some(p @ ("api" | "gemini" | "openai")) => p.into(),
        _ => "cc".into(),
    }
}

async fn run_llm(st: &AppState, system: &str, input: &str, hint: &str, max_tokens: u32) -> R<llm::Out> {
    let db = &st.db;
    match prov(st).as_str() {
        "api" => {
            let key = secrets::get("anthropic_api_key").ok_or("The Anthropic API key is missing (Settings).")?;
            anthropic::enhance(&key, &db.kv_get("anthropic_model").unwrap_or_else(|| anthropic::MODELS[0].into()), input, hint).await
        }
        "gemini" => {
            let key = secrets::get("gemini_api_key").ok_or("The Gemini API key is missing (Settings).")?;
            let model = db.kv_get("gemini_model").unwrap_or_else(|| "gemini-2.5-flash".into());
            llm::gemini(&key, &model, system, input, max_tokens).await
        }
        "openai" => {
            let base = db.kv_get("openai_base_url").unwrap_or_else(|| "https://api.openai.com/v1".into());
            let model = db.kv_get("openai_model").unwrap_or_else(|| "gpt-4o-mini".into());
            llm::openai(&base, &secrets::get("openai_api_key").unwrap_or_default(), &model, system, input, max_tokens).await
        }
        _ => anthropic::enhance_cli(&db.kv_get("claude_model").unwrap_or_else(|| anthropic::CLI_MODELS[0].into()), input, hint).await,
    }
}

fn full_system(hint: &str) -> String {
    if hint.trim().is_empty() { anthropic::SYSTEM.into() } else { format!("{} Workflow: {hint}", anthropic::SYSTEM) }
}

#[tauri::command]
async fn enhance_prompt(st: State<'_, AppState>, idea: String, hint: String) -> R<Enhanced> {
    let o = run_llm(&st, &full_system(&hint), &idea, &hint, 250).await?;
    Ok(Enhanced { text: o.text, tokens: o.tokens, provider: prov(&st) })
}

/// Cheapest possible call with whichever provider is selected.
#[tauri::command]
async fn test_llm(st: State<'_, AppState>) -> R<String> {
    let o = run_llm(&st, "Reply with the single word: ok", "ping", "", 8).await?;
    let _ = o.text;
    Ok(format!("OK ({}){}", prov(&st), o.tokens.map(|t| format!(" · {t} tokens")).unwrap_or_default()))
}

// ---- connection tests

#[derive(Serialize)]
struct VastTest {
    ok: bool,
    message: String,
    endpoint: Option<vast::EndpointInfo>,
    all_endpoints: Vec<String>,
}

#[tauri::command]
async fn test_vast(st: State<'_, AppState>) -> R<VastTest> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    let name = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    let list = st.vast.endpoints(&key).await?;
    let all: Vec<String> = list.iter().map(|x| x.name.clone()).collect();
    match list.into_iter().find(|x| x.name == name) {
        Some(ep) => {
            let msg = if ep.scales_to_zero {
                "Connection OK. The endpoint scales to zero when you are not using it.".to_string()
            } else {
                format!(
                    "Connection OK, BUT the endpoint keeps workers warm (cold_workers={}, min_load={}) and may charge you when idle. Press \"Apply scale-to-zero\".",
                    ep.cold_workers, ep.min_load
                )
            };
            Ok(VastTest { ok: true, message: msg, endpoint: Some(ep), all_endpoints: all })
        }
        None => Ok(VastTest {
            ok: false,
            message: format!("The key works, but no endpoint named \"{name}\" was found. Available: {}", all.join(", ")),
            endpoint: None,
            all_endpoints: all,
        }),
    }
}

#[tauri::command]
async fn test_anthropic(st: State<'_, AppState>) -> R<String> {
    let key = secrets::get("anthropic_api_key").ok_or("The Anthropic API key is missing.")?;
    let model = st.db.kv_get("anthropic_model").unwrap_or_else(|| anthropic::MODELS[0].into());
    anthropic::test(&key, &model).await?;
    Ok(format!("Connection OK ({model})"))
}

#[tauri::command]
async fn apply_scale_to_zero(st: State<'_, AppState>) -> R<String> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    let name = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    let ep = st.vast.endpoints(&key).await?.into_iter().find(|x| x.name == name).ok_or("Endpoint not found")?;
    st.vast.scale_to_zero(&key, ep.id).await?;
    Ok("cold_workers=0, min_load=0 applied.".into())
}

// ---- GPU

#[tauri::command]
async fn gpu_offers(st: State<'_, AppState>, min_vram: f64) -> R<Vec<vast::GpuOffer>> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    st.vast.gpu_offers(&key, min_vram).await
}

/// Points the endpoint's workergroup at the given GPU models. `persist` also stores them as the global choice.
pub async fn apply_gpu_filter(st: &AppState, gpus: &[String], min_vram: f64, dph: f64, persist: bool) -> R<String> {
    // The choice is saved first, so it is not lost when Vast has no endpoint yet; it is pushed to Vast before the next job.
    if persist {
        st.db.kv_set("gpu_selection", &gpus.join(",")).map_err(e)?;
        st.db.kv_set("dph_estimate", &dph.to_string()).map_err(e)?;
    }
    let soft = |m: &str| -> R<String> {
        if persist { Ok(format!("Saved. {m} It will be applied on Vast once that is done (Settings → Create endpoint).")) } else { Err(m.to_string()) }
    };
    let Some(key) = secrets::get("vast_api_key") else { return soft("The Vast API key is missing.") };
    let name = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    let Some(ep) = st.vast.endpoints(&key).await?.into_iter().find(|x| x.name == name) else {
        return soft("No endpoint found on Vast.");
    };
    let Some((wg, sp)) = st.vast.workergroup(&key, ep.id, &name).await? else {
        return soft("No workergroup found for the endpoint.");
    };
    let merged = vast::merge_gpu_filter(&sp, gpus);
    st.vast.set_search_params(&key, ep.id, wg, &merged, min_vram).await?;
    st.db.kv_set("gpu_applied", &gpus.join(",")).map_err(e)?;
    Ok(format!("search_params = {merged}"))
}

#[tauri::command]
async fn apply_gpus(st: State<'_, AppState>, gpus: Vec<String>, min_vram: f64, dph: f64) -> R<String> {
    apply_gpu_filter(&st, &gpus, min_vram, dph, true).await
}

// ---- models / worker install

#[derive(Serialize)]
struct InstallResult {
    script: String,
    download_gb: f64,
    applied: bool,
    message: String,
}

#[tauri::command]
async fn vast_instances(st: State<'_, AppState>) -> R<Vec<vast::InstanceInfo>> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    st.vast.instances(&key).await
}

/// Destroys one rented card (`id`), or all of them when `id` is 0. Also sets the endpoint to scale to zero.
#[tauri::command]
async fn vast_destroy(st: State<'_, AppState>, id: i64) -> R<String> {
    stop_vast(&st, id).await
}

pub async fn stop_vast(st: &AppState, id: i64) -> R<String> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    let endpoint = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    WARM_CANCEL.store(true, Ordering::SeqCst);
    // cancel waiting jobs too, otherwise Vast starts a new card for them right after this one closes
    for f in st.cancels.lock().unwrap().values() { f.store(true, Ordering::Relaxed); }
    let list = st.vast.instances(&key).await?;
    // only this app's own cards (workers of its endpoint); other cards on the account are never touched
    let ours = |i: &vast::InstanceInfo| i.label.starts_with(&format!("{endpoint}:")) || i.label == card::LABEL;
    let mut n = 0;
    for i in list.iter().filter(|i| ours(i) && (id == 0 || i.id == id)) {
        st.vast.destroy_instance(&key, i.id).await?;
        log::info(&format!("vast: destroyed instance {} ({})", i.id, i.gpu));
        n += 1;
    }
    if id != 0 && n == 0 { return Err("That card is not one of this app's workers, so it was not touched.".into()); }
    Ok(format!("{n} card(s) closed."))
}

static WARM_CANCEL: AtomicBool = AtomicBool::new(false);
static WARM_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone)]
struct WarmEvent { state: String, text: String, secs: u64 }

/// "Start card": asks Vast for a worker without sending a job, so the models download now and the first video starts at once.
/// The worker only reports ready after its start-up script (the model downloads) has finished. Costs GPU time while it waits.
#[tauri::command]
async fn warm_up(app: AppHandle) -> R<()> {
    run_warm(&app).await
}

pub fn warm_state() -> &'static str {
    if WARM_RUNNING.load(Ordering::SeqCst) { "preparing" } else { "idle" }
}

pub async fn run_warm(app: &AppHandle) -> R<()> {
    run_warm_with(app, None, None).await
}

/// `workflow`: the mode whose models the card should hold; `gpu`: exact Vast GPU name. Both default to the saved choices.
pub async fn run_warm_with(app: &AppHandle, workflow: Option<String>, gpu: Option<String>) -> R<()> {
    use tauri::Emitter;
    let st = app.state::<AppState>();
    if WARM_RUNNING.swap(true, Ordering::SeqCst) { return Err("Already preparing a card.".into()); }
    WARM_CANCEL.store(false, Ordering::SeqCst);
    let res = async {
        let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
        if st.db.kv_get("vast_mode").as_deref() != Some("serverless") {
            // a card of our own: GPU from the selection (default RTX 4090), models of the last used mode (default Wan 2.2 image→video)
            let gpu = gpu.clone().unwrap_or_default();
            let (wfs, _) = st.load_workflows();
            let pack = workflow.clone().or_else(|| st.db.kv_get("last_workflow")).and_then(|id| wfs.iter().find(|w| w.meta.id == id).map(|w| w.meta.pack.clone())).filter(|p| !p.is_empty()).unwrap_or_else(|| "wan22-i2v".into());
            let cancel = AtomicBool::new(false);
            let (a2, t0) = (app.clone(), std::time::Instant::now());
            let say = move |m: &str| { let _ = a2.emit("warmup", WarmEvent { state: "working".into(), text: m.into(), secs: t0.elapsed().as_secs() }); };
            let packs = [pack];
            let fut = card::ensure(&st, &gpu, &packs, &cancel, &say);
            tokio::pin!(fut);
            loop {
                tokio::select! {
                    r = &mut fut => { let c = r?; card::notify_ready(app, &c); return Ok(true); }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        if WARM_CANCEL.load(Ordering::SeqCst) { cancel.store(true, Ordering::SeqCst); }
                    }
                }
            }
        }
        let endpoint = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
        let want: Vec<String> = st.db.kv_get("gpu_selection").unwrap_or_default().split(',').filter(|x| !x.is_empty()).map(String::from).collect();
        if !want.is_empty() && st.db.kv_get("gpu_applied").unwrap_or_default() != want.join(",") {
            apply_gpu_filter(&st, &want, 24.0, 0.0, false).await?;
        }
        let t0 = std::time::Instant::now();
        let (mut idx, mut attempt) = (0i64, 0u32);
        log::info("warm_up: start");
        loop {
            if WARM_CANCEL.load(Ordering::SeqCst) { return Ok::<bool, String>(false); }
            if t0.elapsed() > std::time::Duration::from_secs(30 * 60) {
                return Err("No card became ready within 30 minutes. Check the card choice, then try again.".into());
            }
            match st.vast.route(&key, &endpoint, idx).await? {
                vast::Route::Ready { .. } => return Ok(true),
                vast::Route::Waiting { request_idx, status } => {
                    idx = request_idx;
                    let _ = app.emit("warmup", WarmEvent { state: "working".into(), text: status, secs: t0.elapsed().as_secs() });
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_secs_f32((1.5 * attempt as f32).min(6.0))).await;
                }
            }
        }
    }
    .await;
    WARM_RUNNING.store(false, Ordering::SeqCst);
    match res {
        Ok(true) => { log::info("warm_up: ready"); let _ = app.emit("warmup", WarmEvent { state: "ready".into(), text: String::new(), secs: 0 }); Ok(()) }
        Ok(false) => { let _ = app.emit("warmup", WarmEvent { state: "cancelled".into(), text: String::new(), secs: 0 }); Ok(()) }
        Err(e) => { log::info(&format!("warm_up: failed: {e}")); let _ = app.emit("warmup", WarmEvent { state: "error".into(), text: e.clone(), secs: 0 }); Err(e) }
    }
}

/// Safety net: closes the app's cards when nothing has been happening for `auto_close_min` minutes (default 10, 0 = never).
pub async fn auto_close_loop(app: AppHandle) {
    let mut last_active = std::time::Instant::now();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let st = app.state::<AppState>();
        let busy = WARM_RUNNING.load(Ordering::SeqCst)
            || st.db.jobs(None).map(|j| j.iter().any(|j| matches!(j.status.as_str(), "queued" | "routing" | "generating" | "downloading"))).unwrap_or(false);
        if busy { last_active = std::time::Instant::now(); continue; }
        let limit: u64 = st.db.kv_get("auto_close_min").and_then(|v| v.parse().ok()).unwrap_or(10);
        if limit == 0 || last_active.elapsed() < std::time::Duration::from_secs(limit * 60) { continue; }
        let Some(key) = secrets::get("vast_api_key") else { continue };
        let endpoint = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
        if let Ok(list) = st.vast.instances(&key).await {
            if list.iter().any(|i| i.label.starts_with(&format!("{endpoint}:")) || i.label == card::LABEL) {
                log::info(&format!("auto-close: nothing happened for {limit} min, closing the card(s)"));
                if let Ok(m) = stop_vast(&st, 0).await {
                    use tauri::Emitter;
                    let _ = app.emit("card-auto-closed", m);
                }
            }
        }
    }
}

#[tauri::command]
fn warm_cancel() { WARM_CANCEL.store(true, Ordering::SeqCst); }

/// Builds the worker install script (selected models + their custom nodes + the local-save node).
/// With `apply`, it is written into a new private copy of the Vast template and the workergroup is pointed at it.
#[tauri::command]
async fn install_on_workers(st: State<'_, AppState>, ids: Vec<String>, apply: bool) -> R<InstallResult> {
    let packs: Vec<Value> = catalog(&st)
        .into_iter()
        .filter(|m| ids.iter().any(|i| Some(i.as_str()) == m["id"].as_str()) && m["in_template"] != true)
        .collect();
    let gb: f64 = packs.iter().map(|m| m["size_gb"].as_f64().unwrap_or(0.0)).sum();
    let script = provision::build_script(&packs, true);
    st.db.kv_set("models_selected", &ids.join(",")).map_err(e)?;
    if !apply {
        return Ok(InstallResult { script, download_gb: gb, applied: false, message: String::new() });
    }

    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    let name = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    let ep = st.vast.endpoints(&key).await?.into_iter().find(|x| x.name == name).ok_or("Endpoint not found")?;
    let wg = st.vast.workergroup_json(&key, ep.id, &name).await?.ok_or("No workergroup found for the endpoint")?;
    let wg_id = wg["id"].as_i64().ok_or("workergroup without an id")?;
    let hash = wg["template_hash"].as_str().ok_or("The workergroup has no template_hash")?.to_string();
    let t = st.vast.template_by_hash(&key, &hash).await?;

    let onstart = provision::patch_onstart(t["onstart"].as_str().unwrap_or(""), &provision::block(&script));
    let mut body = serde_json::Map::new();
    for (to, from) in [
        ("image", "image"), ("tag", "tag"), ("href", "href"), ("repo", "repo"), ("env", "env"),
        ("jup_direct", "jup_direct"), ("ssh_direct", "ssh_direct"), ("use_jupyter_lab", "use_jupyter_lab"),
        ("runtype", "runtype"), ("use_ssh", "use_ssh"), ("jupyter_dir", "jupyter_dir"),
        ("docker_login_repo", "docker_login_repo"), ("extra_filters", "extra_filters"),
        ("readme", "readme"), ("readme_visible", "readme_visible"), ("desc", "desc"),
    ] {
        let mut v = t.get(from).cloned().unwrap_or(Value::Null);
        // Vast returns extra_filters as a JSON string but wants an object when a template is created.
        if from == "extra_filters" {
            if let Value::String(s) = &v {
                v = serde_json::from_str::<Value>(s).ok().filter(|x| x.is_object()).unwrap_or_else(|| json!({}));
            }
        }
        body.insert(to.into(), v);
    }
    let base_name = t["name"].as_str().unwrap_or("ComfyUI Wan 2.2").trim_start_matches("AIVS ").to_string();
    let disk = t["recommended_disk_space"].as_f64().unwrap_or(100.0).max(100.0 + gb);
    body.insert("name".into(), json!(format!("AIVS {base_name}")));
    body.insert("onstart".into(), json!(onstart));
    body.insert("recommended_disk_space".into(), json!(disk));
    body.insert("private".into(), json!(true));
    let new_hash = st.vast.create_template(&key, Value::Object(body)).await?;
    st.vast.set_template_hash(&key, ep.id, wg_id, &new_hash).await?;
    // linking a template resets the workergroup filters (card choice); make the next job or "Prepare now" push them again
    st.db.kv_set("gpu_applied", "").map_err(e)?;
    st.db.kv_set("worker_pack", &new_hash).map_err(e)?;
    Ok(InstallResult {
        script,
        download_gb: gb,
        applied: true,
        message: format!("Created a private template \"AIVS {base_name}\" ({new_hash}) and linked it to the endpoint. It applies to new workers; warm ones keep the old setup until they scale down."),
    })
}

// ---- HuggingFace + custom models

#[derive(Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "lowercase")]
enum HfResult {
    Hits(Vec<hf::Hit>),
    Repo(hf::Repo),
}

/// A link / `org/repo` opens that repo's files; any other text searches HuggingFace.
#[tauri::command]
async fn hf_lookup(query: String, video_only: bool) -> R<HfResult> {
    if query.trim().is_empty() {
        return Err("Type a name or a link.".into());
    }
    match hf::parse_repo(&query) {
        Some(id) => Ok(HfResult::Repo(hf::repo(&id).await?)),
        None => Ok(HfResult::Hits(hf::search(query.trim(), video_only).await?)),
    }
}

#[tauri::command]
fn custom_model_add(
    st: State<AppState>, repo: String, name: String, kind: String, vram_gb: u32, license: String, files: Vec<(String, String, u64)>, insight: Option<Value>,
) -> R<Vec<Value>> {
    let pack = hf::build_pack(&repo, &name, &kind, vram_gb, &license, &files, insight.unwrap_or(Value::Null))?;
    log::info(&format!("custom model added: {repo} ({} files)", files.len()));
    let mut custom: Vec<Value> = st.db.kv_get("custom_models").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    custom.retain(|m| m["id"] != pack["id"]);
    custom.push(pack);
    st.db.kv_set("custom_models", &Value::Array(custom).to_string()).map_err(e)?;
    Ok(catalog(&st))
}

#[tauri::command]
fn custom_model_remove(st: State<AppState>, id: String) -> R<Vec<Value>> {
    let mut custom: Vec<Value> = st.db.kv_get("custom_models").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    custom.retain(|m| m["id"].as_str() != Some(id.as_str()));
    st.db.kv_set("custom_models", &Value::Array(custom).to_string()).map_err(e)?;
    Ok(catalog(&st))
}

/// Copies a ComfyUI workflow (.json) into the user's workflows folder and reloads.
#[tauri::command]
fn workflow_import(st: State<AppState>, path: String) -> R<(Vec<workflows::Workflow>, Vec<String>)> {
    let src = PathBuf::from(&path);
    if !src.extension().map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false) {
        return Err("Pick a .json file (a ComfyUI workflow, \"Export (API)\").".into());
    }
    let text = std::fs::read_to_string(&src).map_err(e)?;
    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).map_err(|x| format!("The file is not valid JSON: {x}"))?;
    let dir = workflows::user_dir(&st.data_dir);
    std::fs::create_dir_all(&dir).map_err(e)?;
    std::fs::copy(&src, dir.join(src.file_name().ok_or("Invalid file")?)).map_err(e)?;
    Ok(st.load_workflows())
}

// ---- first-time Vast setup

/// Creates the scale-to-zero endpoint + workergroup from Vast's official Wan 2.2 template (only what is missing).
#[tauri::command]
async fn setup_endpoint(st: State<'_, AppState>, gpus: Vec<String>) -> R<String> {
    let key = secrets::get("vast_api_key").ok_or("The Vast API key is missing.")?;
    let name = st.db.kv_get("endpoint_name").unwrap_or_else(|| "comfyui-json".into());
    log::info(&format!("setup_endpoint: start (endpoint {name}, gpus {gpus:?})"));
    let mut done: Vec<String> = vec![];
    let mut ep = st.vast.endpoints(&key).await?.into_iter().find(|x| x.name == name);
    if ep.is_none() {
        st.vast.create_endpoint(&key, &name).await?;
        done.push(format!("created the endpoint \"{name}\""));
        ep = st.vast.endpoints(&key).await?.into_iter().find(|x| x.name == name);
    }
    let ep = ep.ok_or("The endpoint was created but has not shown up on Vast yet. Try again in a moment.")?;
    if st.vast.workergroup_json(&key, ep.id, &name).await?.is_none() {
        let hash = st.vast.official_template(&key).await?;
        let base = "verified=true rentable=true rented=false num_gpus=1";
        let sp = if gpus.is_empty() { base.to_string() } else { vast::merge_gpu_filter(base, &gpus) };
        st.vast.create_workergroup(&key, ep.id, &name, &hash, &sp, 24.0).await?;
        st.db.kv_set("worker_pack", &hash).map_err(e)?;
        done.push(format!("created a workergroup with the official template ({hash})"));
    }
    if !gpus.is_empty() {
        st.db.kv_set("gpu_selection", &gpus.join(",")).map_err(e)?;
        st.db.kv_set("gpu_applied", &gpus.join(",")).map_err(e)?;
    }
    let msg = if done.is_empty() { "Everything was already set up on Vast.".to_string() } else { format!("Done: {}.", done.join(" and ")) };
    log::info(&format!("setup_endpoint: {msg}"));
    Ok(msg)
}

// ---- logs

#[tauri::command]
fn logs_tail(lines: usize) -> String {
    log::tail(lines.clamp(10, 2000))
}

#[tauri::command]
fn log_ui(level: String, msg: String) {
    log::write(if level == "error" { "ERROR" } else { "INFO" }, &format!("[ui] {msg}"));
}

/// Writes one self-contained report file (system info, settings, recent jobs, last log lines) and returns its path.
#[tauri::command]
fn logs_report(st: State<AppState>) -> R<String> {
    let dir = log::dir().ok_or("There is no logs folder")?.to_path_buf();
    let now = chrono::Local::now();
    let mut t = String::new();
    t.push_str(&format!("AI Video Studio report\ncreated: {}\nversion: {}\nos: {} {}\n\n", now.to_rfc3339(), env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH));
    t.push_str("== settings (no secrets)\n");
    for k in SETTING_KEYS {
        if let Some(v) = st.db.kv_get(k) {
            t.push_str(&format!("{k} = {v}\n"));
        }
    }
    t.push_str(&format!("\n== keys present: {:?}\n", secrets::status()));
    let (wf, errs) = st.load_workflows();
    t.push_str(&format!("\n== workflows: {}\n", wf.iter().map(|w| w.meta.id.as_str()).collect::<Vec<_>>().join(", ")));
    for er in errs {
        t.push_str(&format!("workflow error: {er}\n"));
    }
    t.push_str("\n== last 25 jobs (newest first)\n");
    for j in st.db.jobs(None).map_err(e)?.into_iter().take(25) {
        let params: String = j.params.chars().take(300).collect();
        t.push_str(&format!(
            "{} | {} | {} | gpu={} | cost={:.3} | prompt={} | err={} | params={}\n",
            j.id.chars().take(8).collect::<String>(), j.status, j.workflow_id, j.gpu, j.est_cost,
            j.prompt.chars().take(80).collect::<String>(), j.error, log::scrub(&params)
        ));
    }
    t.push_str("\n== log (last 800 lines)\n");
    t.push_str(&log::tail(800));
    t.push('\n');
    let path = dir.join(format!("report-{}.txt", now.format("%Y%m%d-%H%M%S")));
    std::fs::write(&path, t).map_err(e)?;
    log::info(&format!("report written: {}", path.display()));
    Ok(path.to_string_lossy().into())
}

#[tauri::command]
fn logs_dir() -> String {
    log::dir().map(|d| d.to_string_lossy().into()).unwrap_or_default()
}

// ---- files / folders

#[tauri::command]
fn reveal(path: String) -> R<()> {
    let p = PathBuf::from(&path);
    if p.is_file() {
        std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn().map_err(e)?;
    } else {
        std::fs::create_dir_all(&p).map_err(e)?;
        std::process::Command::new("explorer").arg(&p).spawn().map_err(e)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct ConnectInfo { extension: String, installer_bat: String, exe: String, claude_code_cmd: String, folder: String }

/// Where the Claude connection files are (shipped inside the installer) and the command for Claude Code.
#[tauri::command]
fn connect_info(app: AppHandle) -> R<ConnectInfo> {
    let clean = |p: PathBuf| p.to_string_lossy().trim_start_matches(r"\?\").to_string();
    let dir = app.path().resource_dir().map_err(e)?.join("connect");
    let exe = clean(std::env::current_exe().map_err(e)?);
    Ok(ConnectInfo {
        extension: clean(dir.join("ai-video-studio.mcpb")),
        installer_bat: clean(dir.join("Connect-Claude.bat")),
        claude_code_cmd: format!("claude mcp add ai-video-studio -- \"{exe}\" --mcp"),
        exe,
        folder: clean(dir),
    })
}

#[tauri::command]
fn export_settings(st: State<AppState>, path: String) -> R<()> {
    let m: BTreeMap<String, String> =
        SETTING_KEYS.iter().filter_map(|k| st.db.kv_get(k).map(|v| (k.to_string(), v))).collect();
    std::fs::write(path, serde_json::to_string_pretty(&m).map_err(e)?).map_err(e)
}

#[tauri::command]
fn import_settings(st: State<AppState>, path: String) -> R<()> {
    let m: BTreeMap<String, String> = serde_json::from_str(&std::fs::read_to_string(path).map_err(e)?).map_err(e)?;
    for (k, v) in m {
        // paths and hosts are never imported: a shared settings file must not redirect an executable folder or the API-key host
        if SETTING_KEYS.contains(&k.as_str()) && !["out_dir", "local_comfy_dir", "openai_base_url", "api_enabled"].contains(&k.as_str()) {
            st.db.kv_set(&k, &v).map_err(e)?;
        }
    }
    Ok(())
}

// ---- hosted providers and chat

#[tauri::command]
async fn hosted_test(provider: String) -> R<String> {
    let secret = hosted::secret_name(&provider).ok_or("Unknown provider")?;
    let key = secrets::get(secret).ok_or("Save the key first.")?;
    hosted::test_key(&provider, &key).await
}

#[tauri::command]
fn chat_providers() -> Vec<chat::ProviderInfo> {
    chat::providers()
}
#[tauri::command]
async fn chat_models(st: State<'_, AppState>, provider: String) -> R<Vec<String>> {
    Ok(chat::list_models(&st, &provider).await)
}
#[tauri::command]
fn chat_folders(st: State<AppState>) -> R<Vec<chat::Folder>> {
    chat::folders(&st.db)
}
#[tauri::command]
fn chat_folder_create(st: State<AppState>, name: String) -> R<Vec<chat::Folder>> {
    chat::folder_create(&st.db, &name)?;
    chat::folders(&st.db)
}
#[tauri::command]
fn chat_folder_rename(st: State<AppState>, id: String, name: String) -> R<Vec<chat::Folder>> {
    chat::folder_rename(&st.db, &id, &name)?;
    chat::folders(&st.db)
}
#[tauri::command]
fn chat_folder_delete(st: State<AppState>, id: String) -> R<Vec<chat::Folder>> {
    chat::folder_delete(&st.db, &id)?;
    chat::folders(&st.db)
}
#[tauri::command]
fn chat_list(st: State<AppState>, query: String) -> R<Vec<chat::Chat>> {
    chat::chats(&st.db, &query)
}
#[tauri::command]
fn chat_new(st: State<AppState>, folder_id: String, provider: String, model: String, mode: String) -> R<chat::Chat> {
    chat::chat_create(&st.db, &folder_id, &provider, &model, &mode)
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn chat_update(st: State<AppState>, id: String, title: String, folder_id: String, provider: String, model: String, system: String, pinned: bool) -> R<()> {
    chat::chat_update(&st.db, &id, &title, &folder_id, &provider, &model, &system, pinned)
}
#[tauri::command]
fn chat_delete(st: State<AppState>, id: String) -> R<()> {
    chat::chat_delete(&st.db, &id)
}
#[tauri::command]
fn chat_messages(st: State<AppState>, id: String) -> R<Vec<chat::Message>> {
    chat::messages(&st.db, &id)
}
#[tauri::command]
fn chat_message_delete(st: State<AppState>, id: String) -> R<()> {
    chat::message_delete(&st.db, &id)
}
#[tauri::command]
async fn chat_send(st: State<'_, AppState>, id: String, text: String) -> R<Vec<chat::Message>> {
    chat::send(&st, &id, &text).await
}

#[tauri::command]
fn editor_url() -> Option<String> {
    editor::url()
}

/// Reads a finished subtitle file (.srt/.vtt) from the app's own folders.
#[tauri::command]
fn text_output(st: State<AppState>, path: String) -> R<String> {
    let real = std::fs::canonicalize(&path).map_err(e)?;
    let ok = [st.videos_dir.clone(), st.data_dir.clone(), st.output_dir()].iter().filter_map(|d| std::fs::canonicalize(d).ok()).any(|d| real.starts_with(d))
        || st.db.projects().map_err(e)?.iter().filter_map(|p| std::fs::canonicalize(st.project_dir(p)).ok()).any(|d| real.starts_with(d));
    let ext = real.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if !ok || !matches!(ext.as_str(), "srt" | "vtt") {
        return Err("Not a subtitle file in the app folders".into());
    }
    let bytes = std::fs::read(&real).map_err(e)?;
    if bytes.len() > 2 << 20 {
        return Err("Subtitle file is too large".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tauri::command]
fn paint_url() -> Option<String> {
    editor::paint_url()
}

/// Reads a finished job's picture for the image editor as a data URL (only files inside the output or data folders).
#[tauri::command]
fn image_data_url(st: State<AppState>, path: String) -> R<String> {
    use base64::Engine;
    let real = std::fs::canonicalize(&path).map_err(e)?;
    let ok = [st.videos_dir.clone(), st.data_dir.clone()].iter().filter_map(|d| std::fs::canonicalize(d).ok()).any(|d| real.starts_with(d))
        || st.db.projects().map_err(e)?.iter().filter_map(|p| std::fs::canonicalize(&p.folder).ok()).any(|d| real.starts_with(d));
    if !ok {
        return Err("That file is outside the app folders".into());
    }
    let mime = match real.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => return Err("Not a picture".into()),
    };
    let bytes = std::fs::read(&real).map_err(e)?;
    if bytes.len() > 40 << 20 {
        return Err("Picture is too large".into());
    }
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

// ---- local generation (setup wizard)

#[tauri::command]
async fn hardware_scan(st: State<'_, AppState>) -> R<local::Hardware> {
    let dir = local::portable_dir(&st);
    tokio::task::spawn_blocking(move || local::scan_hardware(&dir)).await.map_err(e)
}

#[tauri::command]
async fn local_status(st: State<'_, AppState>) -> R<local::Status> {
    Ok(local::status(&st).await)
}

fn setup_flag(st: &AppState) -> Arc<AtomicBool> {
    let f = Arc::new(AtomicBool::new(false));
    st.cancels.lock().unwrap().insert("setup".into(), f.clone());
    f
}

#[tauri::command]
async fn local_install(app: AppHandle, st: State<'_, AppState>) -> R<()> {
    let flag = setup_flag(&st);
    let r = local::install(&app, &st, &flag).await;
    st.cancels.lock().unwrap().remove("setup");
    if let Err(x) = &r {
        log::error(&format!("local install: {x}"));
    }
    r
}

#[tauri::command]
async fn local_install_models(app: AppHandle, st: State<'_, AppState>, ids: Vec<String>) -> R<()> {
    let packs: Vec<Value> = catalog(&st).into_iter().filter(|p| ids.iter().any(|i| p["id"] == *i)).collect();
    if packs.is_empty() {
        return Err("No model selected".into());
    }
    let flag = setup_flag(&st);
    let r = local::install_models(&app, &st, &packs, &flag).await;
    st.cancels.lock().unwrap().remove("setup");
    if let Err(x) = &r {
        log::error(&format!("local models: {x}"));
    }
    r
}

#[tauri::command]
fn setup_cancel(st: State<AppState>) {
    if let Some(f) = st.cancels.lock().unwrap().get("setup") {
        f.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
async fn local_start(st: State<'_, AppState>) -> R<()> {
    local::start(&st).await
}

#[tauri::command]
async fn local_stop(st: State<'_, AppState>) -> R<()> {
    st.local.stop().await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Claude Desktop starts "app.exe --mcp" as its connector: no window, just the MCP protocol on stdin/stdout.
    if std::env::args().any(|a| a == "--mcp") {
        mcp::run();
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            log::init(&data_dir.join("logs"));
            std::panic::set_hook(Box::new(|info| log::error(&format!("PANIC: {info}"))));
            log::info(&format!(
                "=== AI Video Studio {} starting · {} · data: {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                data_dir.display()
            ));
            let mut bundled = app.path().resource_dir()?.join("workflows");
            if !bundled.exists() {
                // `tauri dev` fallback
                bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workflows");
            }
            let videos = app.path().video_dir().unwrap_or_else(|_| data_dir.clone()).join("AI Video Studio");
            let db = Db::open(&data_dir.join("studio.db")).map_err(|x| x.to_string())?;
            chat::init(&db).map_err(|x| x.to_string())?;
            app.manage(AppState {
                db,
                vast: vast::Vast::new(),
                cancels: Mutex::new(HashMap::new()),
                local: local::Local::default(),
                data_dir,
                bundled_dir: bundled,
                videos_dir: videos,
            });
            let h = app.handle().clone();
            app.state::<AppState>().sync_asset_scope(&h);
            tauri::async_runtime::spawn(queue::run_loop(h.clone()));
            tauri::async_runtime::spawn(auto_close_loop(h.clone()));
            // The local API (Claude Desktop extension) is OFF unless the user turns it on in Settings: nothing listens, no token file exists.
            if app.state::<AppState>().db.kv_get("api_enabled").as_deref() == Some("1") {
                api::set_enabled(&h, true);
            } else {
                let _ = std::fs::remove_file(app.state::<AppState>().data_dir.join("api.json"));
            }
            let mut editor_dir = h.path().resource_dir().map(|d| d.join("editor")).unwrap_or_default();
            if !editor_dir.exists() {
                // `tauri dev` fallback
                editor_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("editor-dist");
            }
            tauri::async_runtime::spawn(editor::serve(editor_dir));
            let mut paint_dir = h.path().resource_dir().map(|d| d.join("paint")).unwrap_or_default();
            if !paint_dir.exists() {
                paint_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("paint-dist");
            }
            tauri::async_runtime::spawn(editor::serve_paint(paint_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, reload_workflows, setting_set, secret_set,
            project_create, project_update, project_delete,
            jobs_list, job_enqueue, job_cancel, job_retry, job_delete,
            enhance_prompt, test_llm, test_vast, test_anthropic, apply_scale_to_zero,
            gpu_offers, connect_info, apply_gpus, install_on_workers, vast_instances, vast_destroy, warm_up, warm_cancel, setup_endpoint, logs_tail, log_ui, logs_report, logs_dir, hf_lookup, custom_model_add, custom_model_remove, workflow_import, reveal, export_settings, import_settings,
            hardware_scan, local_status, local_install, local_install_models, setup_cancel, local_start, local_stop,
            hosted_test, chat_providers, chat_models, chat_folders, chat_folder_create, chat_folder_rename, chat_folder_delete, chat_list, chat_new,
            chat_update, chat_delete, chat_messages, chat_message_delete, chat_send, editor_url, paint_url, image_data_url, input_import, text_output
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(app.state::<AppState>().local.stop());
            }
        });
}

#[cfg(test)]
mod media_tests {
    use super::*;

    #[test]
    fn media_keys_cannot_carry_paths() {
        for ok in ["image", "image_end", "audio", "audio_target", "video"] {
            assert!(is_media_key(ok), "{ok}");
        }
        for bad in ["image_..", "image_../x", "audio_C:", "video_a/b", "image_A", "images", "prompt", "image_"] {
            assert!(!is_media_key(bad), "{bad}");
        }
    }
}

#[cfg(test)]
mod stash_tests {
    use super::*;

    #[test]
    fn only_local_drive_paths_pass() {
        for ok in [r"C:\Users\a\x.png", r"\\?\C:\Users\a\x.png"] {
            assert!(is_local_drive_path(Path::new(ok)), "{ok}");
        }
        for bad in [r"\\server\share\a.png", r"\\?\UNC\server\share\a.png", r"\\.\pipe\x.png", "//server/share/a.png", "relative.png", "C:rel.png", "/etc/x.png"] {
            assert!(!is_local_drive_path(Path::new(bad)), "{bad}");
        }
    }
}

#[cfg(test)]
mod pip_tests {
    use super::safe_pip_spec;

    #[test]
    fn only_pinned_names_pass() {
        assert!(safe_pip_spec("faster-whisper==1.2.1"));
        for bad in ["faster-whisper", "x==1;rm", "--index-url=http://a", "a b==1", "git+https://x==1", "-e==1", "x==", "==1", "x==1 --pre"] {
            assert!(!safe_pip_spec(bad), "{bad}");
        }
    }
}
