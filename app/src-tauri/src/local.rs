//! Run generation on this PC: detect the hardware, install a portable ComfyUI, download models, start the server,
//! and run workflows through its HTTP API. Windows + NVIDIA for now.
use crate::AppState;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

pub const PORT: u16 = 8188;
const SEVEN_ZIP_URL: &str = "https://www.7-zip.org/a/7zr.exe";
const COMFY_RELEASE_API: &str = "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest";
const LOCAL_NODE: &str = include_str!("../aivs_save_b64.py");
const NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default)]
pub struct Local {
    child: Mutex<Option<tokio::process::Child>>,
}

impl Local {
    pub async fn stop(&self) {
        if let Some(mut c) = self.child.lock().await.take() {
            let _ = c.kill().await;
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Gpu {
    pub name: String,
    pub vram_mb: u64,
    pub driver: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Hardware {
    pub os: String,
    pub gpus: Vec<Gpu>,
    pub ram_gb: f64,
    pub disk_free_gb: f64,
    pub disk_path: String,
    /// "nvidia" | "amd" | "intel" | "none": which ComfyUI portable build fits (AMD and Intel are experimental)
    pub support: String,
}

#[derive(Serialize, Clone)]
struct Progress {
    step: String,
    pct: f64,
    detail: String,
}

fn progress(app: &AppHandle, step: &str, pct: f64, detail: &str) {
    let _ = app.emit("setup-progress", Progress { step: step.into(), pct, detail: detail.into() });
}

fn hidden(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(NO_WINDOW);
    }
    cmd
}

pub fn parse_nvidia_smi(out: &str) -> Vec<Gpu> {
    out.lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split(',').map(|s| s.trim()).collect();
            if p.len() < 3 || p[0].is_empty() {
                return None;
            }
            Some(Gpu { name: p[0].into(), vram_mb: p[1].parse().ok()?, driver: p[2].into() })
        })
        .collect()
}

/// Picks the ComfyUI build for the first card: NVIDIA, else AMD/Radeon, else Intel Arc/Iris.
fn vendor(nvidia: bool, gpus: &[Gpu]) -> &'static str {
    if nvidia {
        return "nvidia";
    }
    let names = gpus.iter().map(|g| g.name.to_lowercase()).collect::<Vec<_>>().join(" ");
    if names.contains("radeon") || names.contains("amd") {
        "amd"
    } else if names.contains("intel") {
        "intel"
    } else {
        "none"
    }
}

pub fn scan_hardware(target: &Path) -> Hardware {
    let mut gpus = vec![];
    if let Ok(o) = hidden(std::process::Command::new("nvidia-smi").args(["--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"])).output() {
        if o.status.success() {
            gpus = parse_nvidia_smi(&String::from_utf8_lossy(&o.stdout));
        }
    }
    let nvidia = !gpus.is_empty();
    let mut ram_gb = 0.0;
    let mut disk_free_gb = 0.0;
    let drive = target.to_string_lossy().chars().next().filter(|c| c.is_ascii_alphabetic()).unwrap_or('C');
    let script = format!(
        "$g=@(Get-CimInstance Win32_VideoController|Select-Object -ExpandProperty Name); \
         $r=(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory; \
         $d=(Get-PSDrive -Name {drive} -ErrorAction SilentlyContinue).Free; \
         @{{gpus=$g;ram=$r;free=$d}}|ConvertTo-Json -Compress"
    );
    if let Ok(o) = hidden(std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script])).output() {
        if let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) {
            ram_gb = v["ram"].as_f64().unwrap_or(0.0) / 1e9;
            disk_free_gb = v["free"].as_f64().unwrap_or(0.0) / 1e9;
            let names: Vec<String> = match &v["gpus"] {
                Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
                Value::String(s) => vec![s.clone()],
                _ => vec![],
            };
            let real = |n: &str| {
                let l = n.to_lowercase();
                !(l.contains("microsoft") || l.contains("basic") || l.contains("virtual") || l.contains("remote"))
            };
            if !nvidia {
                gpus = names.into_iter().filter(|n| real(n)).map(|name| Gpu { name, vram_mb: 0, driver: String::new() }).collect();
            }
        }
    }
    Hardware {
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        support: vendor(nvidia, &gpus).into(),
        gpus,
        ram_gb: (ram_gb * 10.0).round() / 10.0,
        disk_free_gb: (disk_free_gb * 10.0).round() / 10.0,
        disk_path: format!("{drive}:\\"),
    }
}

// ---- paths

/// Where the portable ComfyUI lives: the user's own folder (setting `local_comfy_dir`) or our default.
pub fn portable_dir(st: &AppState) -> PathBuf {
    match st.db.kv_get("local_comfy_dir") {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => st.data_dir.join("comfyui").join("ComfyUI_windows_portable"),
    }
}
fn python(root: &Path) -> PathBuf {
    root.join("python_embeded").join("python.exe")
}
fn comfy(root: &Path) -> PathBuf {
    root.join("ComfyUI")
}
pub fn installed(root: &Path) -> bool {
    python(root).exists() && comfy(root).join("main.py").exists()
}

async fn server_up() -> bool {
    reqwest::Client::new().get(format!("http://127.0.0.1:{PORT}/system_stats")).timeout(Duration::from_secs(2)).send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

#[derive(Serialize)]
pub struct Status {
    pub installed: bool,
    pub running: bool,
    pub dir: String,
    pub port: u16,
    pub models: Vec<String>,
}

pub async fn status(st: &AppState) -> Status {
    let root = portable_dir(st);
    let inst = installed(&root);
    Status {
        installed: inst,
        running: server_up().await,
        dir: root.to_string_lossy().into(),
        port: PORT,
        models: if inst { installed_files(&root) } else { vec![] },
    }
}

fn installed_files(root: &Path) -> Vec<String> {
    let mut out = vec![];
    for sub in ["checkpoints", "diffusion_models", "text_encoders", "vae", "loras"] {
        if let Ok(rd) = std::fs::read_dir(comfy(root).join("models").join(sub)) {
            for f in rd.flatten() {
                if f.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false) && !f.file_name().to_string_lossy().ends_with(".part") {
                    out.push(format!("{sub}/{}", f.file_name().to_string_lossy()));
                }
            }
        }
    }
    out
}

// ---- download

fn client() -> reqwest::Client {
    reqwest::Client::builder().user_agent("AI-Video-Studio").build().expect("http client")
}

/// How long a download may go without receiving a single byte before it is retried.
const STALL: Duration = Duration::from_secs(30);
const ATTEMPTS: u32 = 12;

/// Streams `url` to `dest` (via a .part file), reporting progress. Skips when the file already exists.
/// A stalled or broken connection is retried and continues from the bytes already on disk (HTTP Range).
async fn download(app: &AppHandle, step: &str, url: &str, dest: &Path, cancel: &AtomicBool) -> Result<(), String> {
    download_with(&|pct, detail| progress(app, step, pct, detail), url, dest, cancel, STALL).await
}

async fn download_with(report: &(dyn Fn(f64, &str) + Sync), url: &str, dest: &Path, cancel: &AtomicBool, stall: Duration) -> Result<(), String> {
    if dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(());
    }
    if let Some(p) = dest.parent() {
        tokio::fs::create_dir_all(p).await.map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut last_err = String::new();
    for attempt in 1..=ATTEMPTS {
        match download_once(report, url, &tmp, &name, cancel, stall).await {
            Ok(()) => return tokio::fs::rename(&tmp, dest).await.map_err(|e| e.to_string()),
            Err(e) if e == "cancelled" => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(e);
            }
            Err(e) => {
                crate::log::error(&format!("download {name} attempt {attempt}/{ATTEMPTS}: {e}"));
                report(0.0, &format!("{name} · connection problem, retrying ({attempt}/{ATTEMPTS})"));
                last_err = e;
                tokio::time::sleep(if stall < Duration::from_secs(5) { Duration::from_millis(50) } else { Duration::from_secs((attempt as u64 * 2).min(15)) }).await;
            }
        }
    }
    Err(format!("Could not download {name} after {ATTEMPTS} tries: {last_err}"))
}

async fn download_once(report: &(dyn Fn(f64, &str) + Sync), url: &str, tmp: &Path, name: &str, cancel: &AtomicBool, stall: Duration) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let have = tokio::fs::metadata(tmp).await.map(|m| m.len()).unwrap_or(0);
    let mut rq = client().get(url);
    if have > 0 {
        rq = rq.header("Range", format!("bytes={have}-"));
    }
    let r = tokio::time::timeout(Duration::from_secs(60), rq.send()).await.map_err(|_| "no response from the server".to_string())?.map_err(|e| e.to_string())?;
    let code = r.status();
    if code.as_u16() == 416 {
        // the file on disk is already complete
        return Ok(());
    }
    if !code.is_success() {
        return Err(format!("HTTP {code}"));
    }
    let resumed = code.as_u16() == 206;
    let total = r.content_length().unwrap_or(0) + if resumed { have } else { 0 };
    let mut f = if resumed {
        tokio::fs::OpenOptions::new().append(true).open(tmp).await.map_err(|e| e.to_string())?
    } else {
        tokio::fs::File::create(tmp).await.map_err(|e| e.to_string())?
    };
    let mut got = if resumed { have } else { 0 };
    let mut last = Instant::now();
    let mut s = r.bytes_stream();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let chunk = match tokio::time::timeout(stall, s.next()).await {
            Err(_) => return Err(format!("no data for {} seconds", stall.as_secs())),
            Ok(None) => break,
            Ok(Some(c)) => c.map_err(|e| e.to_string())?,
        };
        got += chunk.len() as u64;
        f.write_all(&chunk).await.map_err(|e| e.to_string())?;
        if last.elapsed() > Duration::from_millis(300) {
            last = Instant::now();
            let pct = if total > 0 { got as f64 * 100.0 / total as f64 } else { 0.0 };
            report(pct, &format!("{name} · {:.2} / {:.2} GB", got as f64 / 1e9, total as f64 / 1e9));
        }
    }
    f.flush().await.map_err(|e| e.to_string())?;
    if total > 0 && got < total {
        return Err(format!("connection closed early at {got} of {total} bytes"));
    }
    Ok(())
}

// ---- install ComfyUI

pub async fn install(app: &AppHandle, st: &AppState, cancel: &AtomicBool) -> Result<(), String> {
    let root = portable_dir(st);
    if installed(&root) {
        progress(app, "comfy", 100.0, "already installed");
        return install_aivs_node(&root);
    }
    let base = root.parent().ok_or("bad install folder")?.to_path_buf();
    tokio::fs::create_dir_all(&base).await.map_err(|e| e.to_string())?;
    let hw = scan_hardware(&base);
    if hw.support == "none" {
        return Err("No supported graphics card found (NVIDIA, AMD or Intel Arc). Use a cloud GPU provider instead.".into());
    }
    let asset = format!("ComfyUI_windows_portable_{}.7z", hw.support);
    if hw.disk_free_gb > 0.0 && hw.disk_free_gb < 12.0 {
        return Err(format!("Not enough free disk space on {}: {:.1} GB free, at least 12 GB needed for ComfyUI (models need more).", hw.disk_path, hw.disk_free_gb));
    }

    progress(app, "tools", 0.0, "7-Zip");
    let sz = base.join("7zr.exe");
    download(app, "tools", SEVEN_ZIP_URL, &sz, cancel).await?;

    progress(app, "comfy", 0.0, "looking up the latest ComfyUI");
    let rel: Value = client().get(COMFY_RELEASE_API).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let url = rel["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"] == asset.as_str()))
        .and_then(|x| x["browser_download_url"].as_str())
        .ok_or("Could not find the ComfyUI Windows package in the latest release.")?
        .to_string();
    let archive = base.join(&asset);
    download(app, "comfy", &url, &archive, cancel).await?;

    progress(app, "extract", 0.0, "unpacking (a few minutes)");
    let out = tokio::process::Command::new(&sz)
        .arg("x")
        .arg(&archive)
        .arg(format!("-o{}", base.to_string_lossy()))
        .arg("-y")
        .creation_flags_hidden()
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("Unpacking failed: {}", String::from_utf8_lossy(&out.stdout).lines().last().unwrap_or("")));
    }
    let _ = tokio::fs::remove_file(&archive).await;
    if !installed(&root) {
        return Err("ComfyUI was unpacked but the expected folders are missing.".into());
    }
    install_aivs_node(&root)?;
    progress(app, "comfy", 100.0, "done");
    Ok(())
}

trait HiddenExt {
    fn creation_flags_hidden(&mut self) -> &mut Self;
}
impl HiddenExt for tokio::process::Command {
    fn creation_flags_hidden(&mut self) -> &mut Self {
        #[cfg(windows)]
        self.creation_flags(NO_WINDOW);
        self
    }
}

/// The small custom node that lets workflows take a base64 image (image-to-video).
fn install_aivs_node(root: &Path) -> Result<(), String> {
    let d = comfy(root).join("custom_nodes").join("aivs_save_b64");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    std::fs::write(d.join("__init__.py"), LOCAL_NODE).map_err(|e| e.to_string())
}

// ---- models

pub async fn install_models(app: &AppHandle, st: &AppState, packs: &[Value], cancel: &AtomicBool) -> Result<(), String> {
    let root = portable_dir(st);
    if !installed(&root) {
        return Err("ComfyUI is not installed yet.".into());
    }
    let total: usize = packs.iter().map(|p| p["files"].as_array().map(|a| a.len()).unwrap_or(0)).sum();
    let mut done = 0usize;
    for pack in packs {
        for n in pack["nodes"].as_array().cloned().unwrap_or_default() {
            let (repo, name) = (n["repo"].as_str().unwrap_or(""), n["name"].as_str().unwrap_or(""));
            if !repo.is_empty() && !name.is_empty() {
                progress(app, "nodes", 0.0, name);
                install_node(&root, repo, name, n["rev"].as_str().unwrap_or(""), cancel, app).await?;
            }
        }
        // plain pip packages a pack needs (pinned name==version only, so nothing else can be smuggled into the command line)
        // "pip_nodeps": packages installed without their own dependencies (when those would pull heavy or conflicting extras)
        let pip_list: Vec<(Value, bool)> = pack["pip"].as_array().cloned().unwrap_or_default().into_iter().map(|p| (p, false))
            .chain(pack["pip_nodeps"].as_array().cloned().unwrap_or_default().into_iter().map(|p| (p, true))).collect();
        for (pkg, nodeps) in pip_list {
            let pkg = pkg.as_str().unwrap_or("");
            if !crate::safe_pip_spec(pkg) {
                return Err(format!("Refusing pip package: {pkg}"));
            }
            progress(app, "nodes", 0.0, &format!("python package {pkg}"));
            let out = tokio::process::Command::new(python(&root)).args(["-s", "-m", "pip", "install", "-q"]).args(if nodeps { vec!["--no-deps"] } else { vec![] }).arg(pkg).creation_flags_hidden().output().await.map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!("pip install {pkg} failed: {}", String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>()));
            }
        }
        for f in pack["files"].as_array().cloned().unwrap_or_default() {
            let (url, dir) = (f["url"].as_str().unwrap_or(""), f["dir"].as_str().unwrap_or(""));
            if url.is_empty() || dir.contains("..") || dir.starts_with('/') || dir.starts_with('\\') {
                return Err(format!("Refusing model entry with a bad folder: {dir}"));
            }
            let file = url.rsplit('/').next().unwrap_or("model.safetensors");
            if file.contains("..") || file.contains('\\') {
                return Err(format!("Refusing model entry with a bad file name: {file}"));
            }
            // "@custom_nodes/..." = relative to the ComfyUI folder itself (a few node packs keep their weights next to the code)
            let base = match dir.strip_prefix('@') {
                Some(rel) if rel.starts_with("custom_nodes/") => comfy(&root).join(rel),
                Some(_) => return Err(format!("Refusing model entry with a bad folder: {dir}")),
                None => comfy(&root).join("models").join(dir),
            };
            let dest = base.join(file);
            progress(app, "models", done as f64 * 100.0 / total.max(1) as f64, file);
            download(app, "models", url, &dest, cancel).await?;
            done += 1;
        }
    }
    progress(app, "models", 100.0, "done");
    Ok(())
}

async fn install_node(root: &Path, repo: &str, name: &str, rev: &str, cancel: &AtomicBool, app: &AppHandle) -> Result<(), String> {
    if !repo.starts_with("https://github.com/") || name.contains(['/', '\\', '.']) {
        return Err(format!("Refusing custom node {repo}"));
    }
    let dest = comfy(root).join("custom_nodes").join(name);
    if dest.exists() {
        return Ok(());
    }
    let zip_path = comfy(root).join("custom_nodes").join(format!("{name}.zip"));
    // a pinned commit when the pack names one (a later change upstream cannot reach users), else the newest code
    let at = if rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit()) { rev } else { "HEAD" };
    download(app, "nodes", &format!("{}/archive/{at}.zip", repo.trim_end_matches('/')), &zip_path, cancel).await?;
    let (z, d) = (zip_path.clone(), dest.clone());
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut ar = zip::ZipArchive::new(std::fs::File::open(&z).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for i in 0..ar.len() {
            let mut f = ar.by_index(i).map_err(|e| e.to_string())?;
            let Some(rel) = f.enclosed_name() else { continue };
            let rel: PathBuf = rel.components().skip(1).collect();
            if rel.as_os_str().is_empty() {
                continue;
            }
            let target = d.join(rel);
            if f.is_dir() {
                std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = target.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::io::copy(&mut f, &mut std::fs::File::create(&target).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = std::fs::remove_file(&zip_path);
    // some packs only ship a variant file (e.g. Frame-Interpolation: requirements-no-cupy.txt)
    let req = ["requirements.txt", "requirements-no-cupy.txt"].iter().map(|f| dest.join(f)).find(|p| p.exists()).unwrap_or_else(|| dest.join("requirements.txt"));
    if req.exists() {
        progress(app, "nodes", 50.0, &format!("{name}: python packages"));
        let _ = tokio::process::Command::new(python(root)).args(["-s", "-m", "pip", "install", "-q", "-r"]).arg(&req).creation_flags_hidden().output().await;
    }
    Ok(())
}

// ---- server

pub async fn start(st: &AppState) -> Result<(), String> {
    // keep our helper nodes current for installs made by an older version
    {
        let root = portable_dir(st);
        if installed(&root) {
            let _ = install_aivs_node(&root);
        }
    }
    if server_up().await {
        return Ok(());
    }
    let root = portable_dir(st);
    if !installed(&root) {
        return Err("ComfyUI is not installed yet. Run the setup wizard first.".into());
    }
    let mut guard = st.local.child.lock().await;
    if let Some(mut c) = guard.take() {
        let _ = c.kill().await;
    }
    let child = tokio::process::Command::new(python(&root))
        .current_dir(&root)
        .args(["-s", r"ComfyUI\main.py", "--windows-standalone-build", "--listen", "127.0.0.1", "--port", &PORT.to_string(), "--disable-auto-launch"])
        .kill_on_drop(true)
        .creation_flags_hidden()
        .spawn()
        .map_err(|e| format!("Could not start ComfyUI: {e}"))?;
    *guard = Some(child);
    drop(guard);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(240) {
        if server_up().await {
            crate::log::info("local ComfyUI is up");
            return Ok(());
        }
        let mut g = st.local.child.lock().await;
        if let Some(c) = g.as_mut() {
            if let Ok(Some(code)) = c.try_wait() {
                return Err(format!("ComfyUI exited at startup (code {code}). Check that your NVIDIA driver is up to date."));
            }
        }
        drop(g);
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    Err("ComfyUI did not start within 4 minutes.".into())
}

// ---- run a workflow

/// Pulls (filename, subfolder, type) of the produced video out of a /history entry.
pub fn find_output(entry: &Value) -> Option<(String, String, String)> {
    let outs = entry["outputs"].as_object()?;
    let mut best: Option<(String, String, String)> = None;
    for node in outs.values() {
        for key in ["images", "gifs", "videos", "audio", "text", "files", "meshes", "3d", "glb", "subtitles"] {
            for f in node[key].as_array().cloned().unwrap_or_default() {
                let name = f["filename"].as_str().unwrap_or("");
                if f["type"].as_str().unwrap_or("output") != "output" {
                    continue;
                }
                let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
                let t = (name.to_string(), f["subfolder"].as_str().unwrap_or("").to_string(), "output".to_string());
                if matches!(ext.as_str(), "mp4" | "webm" | "mov" | "mp3" | "flac" | "wav" | "ogg" | "opus" | "srt" | "vtt" | "glb" | "obj" | "ply") {
                    return Some(t);
                }
                if best.is_none() && matches!(ext.as_str(), "gif" | "webp" | "png" | "jpg" | "jpeg") {
                    best = Some(t);
                }
            }
        }
    }
    best
}

/// The Vast worker swaps "__RANDOM_INT__" for a random seed; a local ComfyUI does not, so do it here.
pub fn randomize_seeds(v: &mut Value) {
    match v {
        Value::String(s) if s == "__RANDOM_INT__" => *v = json!(uuid::Uuid::new_v4().as_u128() as u32 as i64),
        Value::Array(a) => a.iter_mut().for_each(randomize_seeds),
        Value::Object(m) => m.values_mut().for_each(randomize_seeds),
        _ => {}
    }
}

/// Runs `workflow` on the local ComfyUI. Returns the file bytes and its extension.
pub async fn generate(st: &AppState, workflow: &Value, cancel: &AtomicBool, tick: &(dyn Fn(u64) + Sync)) -> Result<(Vec<u8>, String), String> {
    start(st).await?;
    generate_at(&format!("http://127.0.0.1:{PORT}"), None, workflow, cancel, tick).await
}

/// Same, against any ComfyUI (a rented Vast card: `token` is sent as a bearer token).
pub async fn generate_at(base: &str, token: Option<&str>, workflow: &Value, cancel: &AtomicBool, tick: &(dyn Fn(u64) + Sync)) -> Result<(Vec<u8>, String), String> {
    let c = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().map_err(|e| e.to_string())?;
    let auth = |rb: reqwest::RequestBuilder| match token { Some(t) => rb.bearer_auth(t), None => rb };
    let mut workflow = workflow.clone();
    randomize_seeds(&mut workflow);
    let r = auth(c.post(format!("{base}/prompt"))).json(&json!({"prompt": workflow, "client_id": uuid::Uuid::new_v4().to_string()})).send().await.map_err(|e| e.to_string())?;
    let status = r.status();
    let body: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let err = body["error"]["message"].as_str().unwrap_or("rejected");
        let nodes = body["node_errors"].to_string();
        return Err(format!("ComfyUI rejected the workflow: {err} {}", if nodes == "{}" || nodes == "null" { String::new() } else { nodes }));
    }
    let id = body["prompt_id"].as_str().ok_or("ComfyUI returned no prompt id")?.to_string();
    let t0 = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = auth(c.post(format!("{base}/interrupt"))).send().await;
            return Err("cancelled".into());
        }
        tick(t0.elapsed().as_secs());
        if t0.elapsed() > Duration::from_secs(60 * 60) {
            return Err("Generation took longer than an hour.".into());
        }
        if let Ok(r) = auth(c.get(format!("{base}/history/{id}"))).send().await {
            if let Ok(h) = r.json::<Value>().await {
                if let Some(e) = h.get(&id) {
                    if e["status"]["status_str"] == "error" {
                        let msg = e["status"]["messages"].as_array().and_then(|m| m.iter().find(|x| x[0] == "execution_error")).map(|x| x[1].to_string()).unwrap_or_default();
                        return Err(format!("ComfyUI error: {msg}"));
                    }
                    if let Some((name, sub, ty)) = find_output(e) {
                        let bytes = auth(c.get(format!("{base}/view")))
                            .query(&[("filename", &name), ("subfolder", &sub), ("type", &ty)])
                            .send()
                            .await
                            .map_err(|e| e.to_string())?
                            .bytes()
                            .await
                            .map_err(|e| e.to_string())?;
                        return Ok((bytes.to_vec(), name.rsplit('.').next().unwrap_or("mp4").to_lowercase()));
                    }
                    if e["status"]["completed"] == true {
                        return Err("ComfyUI finished but produced no video file.".into());
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_smi_output_is_parsed() {
        let g = parse_nvidia_smi("NVIDIA GeForce RTX 4060, 8188, 576.02\nNVIDIA GeForce RTX 3090, 24576, 576.02\n");
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].name, "NVIDIA GeForce RTX 4060");
        assert_eq!(g[0].vram_mb, 8188);
        assert!(parse_nvidia_smi("garbage").is_empty());
    }

    #[test]
    fn random_seed_placeholders_become_numbers() {
        let mut w = json!({"10": {"inputs": {"noise_seed": "__RANDOM_INT__", "other": ["1", 0]}}, "11": {"inputs": {"seed": 5}}});
        randomize_seeds(&mut w);
        assert!(w["10"]["inputs"]["noise_seed"].is_i64());
        assert_eq!(w["11"]["inputs"]["seed"], json!(5));
        assert!(!w.to_string().contains("__RANDOM_INT__"));
    }

    #[test]
    fn video_is_found_in_history() {
        let h = json!({"outputs": {"12": {"images": [{"filename": "ComfyUI_00001_.png", "subfolder": "", "type": "temp"}]},
                                  "98": {"images": [{"filename": "video_00001_.mp4", "subfolder": "video", "type": "output"}], "animated": [true]}}});
        assert_eq!(find_output(&h), Some(("video_00001_.mp4".into(), "video".into(), "output".into())));
        assert_eq!(find_output(&json!({"outputs": {}})), None);
        let a = json!({"outputs": {"8": {"audio": [{"filename": "ComfyUI_00001_.mp3", "subfolder": "audio", "type": "output"}]}}});
        assert_eq!(find_output(&a), Some(("ComfyUI_00001_.mp3".into(), "audio".into(), "output".into())));
    }
}

#[cfg(test)]
mod download_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves 3000 bytes: the first connection stalls after 1000 bytes, the next honours Range.
    #[tokio::test]
    async fn stalled_download_resumes_from_the_bytes_on_disk() {
        let data: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let d2 = data.clone();
        tokio::spawn(async move {
            let mut n = 0;
            loop {
                let (mut sock, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 4096];
                let k = sock.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..k]).to_string();
                n += 1;
                let from = req.to_lowercase().split("range: bytes=").nth(1).and_then(|r| r.split('-').next()).and_then(|v| v.trim().parse::<usize>().ok());
                match from {
                    Some(f) => {
                        let body = &d2[f..];
                        let h = format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {f}-2999/3000\r\nConnection: close\r\n\r\n", body.len());
                        sock.write_all(h.as_bytes()).await.unwrap();
                        sock.write_all(body).await.unwrap();
                    }
                    None => {
                        let h = format!("HTTP/1.1 200 OK\r\nContent-Length: 3000\r\nConnection: close\r\n\r\n");
                        sock.write_all(h.as_bytes()).await.unwrap();
                        sock.write_all(&d2[..1000]).await.unwrap();
                        tokio::time::sleep(Duration::from_secs(5)).await; // stall
                    }
                }
                let _ = n;
            }
        });
        let dir = std::env::temp_dir().join(format!("aivs_dl_{}", uuid::Uuid::new_v4()));
        let dest = dir.join("model.bin");
        let cancel = AtomicBool::new(false);
        download_with(&|_, _| {}, &format!("http://127.0.0.1:{port}/m.bin"), &dest, &cancel, Duration::from_millis(400)).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert!(!dest.with_extension("part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod smoke {
    #[test]
    #[ignore = "reads the real machine"]
    fn scan_this_machine() {
        let h = super::scan_hardware(std::path::Path::new("C:\\"));
        println!("{h:?}");
        assert!(!h.os.is_empty());
    }
}
