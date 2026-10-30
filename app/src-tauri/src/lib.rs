mod anthropic;
mod api;
mod chat;
mod hf;
mod hosted;
mod log;
mod db;
mod llm;
mod local;
mod provision;
mod queue;
mod secrets;
mod vast;
mod workflows;

use db::{Db, Job, Project};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
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
fn catalog(st: &AppState) -> Vec<Value> {
    let mut cat: Vec<Value> = serde_json::from_str(include_str!("../models.json")).unwrap_or_default();
    let custom: Vec<Value> = st.db.kv_get("custom_models").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    cat.extend(custom);
    cat
}

const SETTING_KEYS: &[&str] = &[
    "lang", "endpoint_name", "out_dir", "anthropic_model", "dph_estimate", "gpu_selection", "last_project",
    "last_workflow", "models_selected", "enhance_provider", "claude_model", "delivery", "worker_pack", "gemini_model", "theme", "openai_base_url", "openai_model", "provider", "local_comfy_dir", "setup_done",
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

#[tauri::command]
fn setting_set(st: State<AppState>, key: String, value: String) -> R<()> {
    if !SETTING_KEYS.contains(&key.as_str()) {
        return Err(format!("unknown setting {key}"));
    }
    st.db.kv_set(&key, &value).map_err(e)
}

#[tauri::command]
fn secret_set(name: String, value: String) -> R<BTreeMap<String, bool>> {
    secrets::set(&name, &value).map_err(e)?;
    Ok(secrets::status())
}

// ---- projects

#[tauri::command]
fn project_create(st: State<AppState>, name: String, folder: String) -> R<Vec<Project>> {
    if name.trim().is_empty() {
        return Err("The name is empty".into());
    }
    if !folder.trim().is_empty() {
        std::fs::create_dir_all(folder.trim()).map_err(|x| format!("The folder cannot be created: {x}"))?;
    }
    st.db.create_project(&name, &folder).map_err(e)?;
    st.projects()
}

#[tauri::command]
fn project_update(st: State<AppState>, id: String, name: String, folder: String, suffix: String, sidecar: bool) -> R<Vec<Project>> {
    if name.trim().is_empty() {
        return Err("The name is empty".into());
    }
    if !folder.trim().is_empty() {
        std::fs::create_dir_all(folder.trim()).map_err(|x| format!("The folder cannot be created: {x}"))?;
    }
    st.db.update_project(&id, &name, &folder, &suffix, sidecar).map_err(e)?;
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
    let image_keys: Vec<String> = p.keys().filter(|k| *k == "image" || k.starts_with("image_")).cloned().collect();
    for k in image_keys {
        let Some(Value::String(b64)) = p.get(&k).cloned() else { continue };
        if b64.is_empty() {
            p.remove(&k);
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
                let _ = std::fs::remove_file(v);
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
    st.vast.set_search_params(&key, wg, &merged, min_vram).await?;
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
        body.insert(to.into(), t.get(from).cloned().unwrap_or(Value::Null));
    }
    let base_name = t["name"].as_str().unwrap_or("ComfyUI Wan 2.2").trim_start_matches("AIVS ").to_string();
    let disk = t["recommended_disk_space"].as_f64().unwrap_or(100.0).max(100.0 + gb);
    body.insert("name".into(), json!(format!("AIVS {base_name}")));
    body.insert("onstart".into(), json!(onstart));
    body.insert("recommended_disk_space".into(), json!(disk));
    body.insert("private".into(), json!(true));
    let new_hash = st.vast.create_template(&key, Value::Object(body)).await?;
    st.vast.set_template_hash(&key, wg_id, &new_hash).await?;
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
        if SETTING_KEYS.contains(&k.as_str()) && k != "out_dir" {
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
fn chat_new(st: State<AppState>, folder_id: String, provider: String, model: String) -> R<chat::Chat> {
    chat::chat_create(&st.db, &folder_id, &provider, &model)
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            tauri::async_runtime::spawn(queue::run_loop(h.clone()));
            tauri::async_runtime::spawn(api::serve(h));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, reload_workflows, setting_set, secret_set,
            project_create, project_update, project_delete,
            jobs_list, job_enqueue, job_cancel, job_retry, job_delete,
            enhance_prompt, test_llm, test_vast, test_anthropic, apply_scale_to_zero,
            gpu_offers, apply_gpus, install_on_workers, setup_endpoint, logs_tail, log_ui, logs_report, logs_dir, hf_lookup, custom_model_add, custom_model_remove, workflow_import, reveal, export_settings, import_settings,
            hardware_scan, local_status, local_install, local_install_models, setup_cancel, local_start, local_stop,
            hosted_test, chat_providers, chat_models, chat_folders, chat_folder_create, chat_folder_rename, chat_folder_delete, chat_list, chat_new,
            chat_update, chat_delete, chat_messages, chat_message_delete, chat_send
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(app.state::<AppState>().local.stop());
            }
        });
}
