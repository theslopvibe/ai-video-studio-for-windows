//! HuggingFace lookup (public API, no key) and custom model packs.
//! A custom pack has the same shape as an entry in models.json, so the existing worker install handles it.
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://huggingface.co/api/models";
pub const DIRS: &[&str] = &[
    "diffusion_models", "checkpoints", "vae", "text_encoders", "loras", "clip_vision", "controlnet", "upscale_models", "unet",
];
// pickle formats (.ckpt/.pt/.pth) can run code when loaded, so user-imported packs only take these two
const WEIGHT_EXT: &[&str] = &[".safetensors", ".gguf"];

#[derive(Serialize)]
pub struct Hit {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub tag: String,
}

#[derive(Serialize)]
pub struct RepoFile {
    pub path: String,
    pub size: u64,
    pub dir: String,
    /// fp8 / fp16 / bf16 / fp32 / Q4_K_M … ("" = unknown)
    pub precision: String,
    /// Best variant of its group (e.g. the fp8 file when an fp16 copy exists).
    pub recommended: bool,
}

/// What the app worked out about a model from its HuggingFace metadata and file names.
/// Quality and speed are rough estimates (1-5), explained by `basis`.
#[derive(Serialize, Default, Clone)]
pub struct Insight {
    pub kind: String,
    pub modes: Vec<String>,
    pub family: String,
    pub params_b: f64,
    pub precision: String,
    pub vram_gb: u32,
    pub quality: u8,
    pub speed: u8,
    /// "family" (known model family) or "size" (only judged from its size)
    pub basis: String,
    pub commercial: Option<bool>,
    /// typical settings: resolution step, frame rule, fps, steps (text, same in both languages)
    pub typical: Vec<(String, String)>,
    /// ids of bundled workflows that can use this model right away
    pub workflows: Vec<String>,
    /// too many variants to pre-select: the user must choose
    pub too_many: bool,
}

#[derive(Serialize)]
pub struct Repo {
    pub id: String,
    pub license: String,
    pub gated: bool,
    /// diffusers-style repo (model_index.json): ComfyUI cannot load it directly.
    pub diffusers: bool,
    pub files: Vec<RepoFile>,
    pub insight: Insight,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(25)).user_agent("AIVideoStudio").build().map_err(|e| e.to_string())
}

/// Accepts `org/repo`, `https://huggingface.co/org/repo[/tree|blob|resolve/...]`. None = it is a search phrase.
pub fn parse_repo(input: &str) -> Option<String> {
    let t = input.trim();
    let rest = ["https://huggingface.co/", "http://huggingface.co/", "huggingface.co/"].iter().find_map(|p| t.strip_prefix(p));
    let is_link = rest.is_some();
    let rest = rest.unwrap_or(t).trim_end_matches('/');
    let mut it = rest.split('/');
    let (a, b) = (it.next()?, it.next()?);
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    // A bare name must be exactly `org/repo`; a link may continue with /tree/..., /resolve/... etc.
    if ok(a) && ok(b) && (is_link || it.next().is_none()) { Some(format!("{a}/{b}")) } else { None }
}

/// Best-guess ComfyUI folder for a weight file.
pub fn guess_dir(path: &str) -> &'static str {
    let p = path.to_lowercase();
    if p.contains("lora") { "loras" }
    else if p.contains("vae") { "vae" }
    else if p.contains("clip_vision") || p.contains("clip-vision") { "clip_vision" }
    else if p.contains("text_encoder") || p.contains("umt5") || p.contains("t5xxl") || p.contains("t5_xxl") || p.contains("clip_l") || p.contains("clip_g") { "text_encoders" }
    else if p.contains("controlnet") { "controlnet" }
    else if p.contains("upscale") || p.contains("esrgan") { "upscale_models" }
    else if p.contains("diffusion_models") || p.contains("unet") || p.contains("transformer") || p.contains("wan2") || p.contains("hunyuan") || p.ends_with(".gguf") { "diffusion_models" }
    else { "checkpoints" }
}

fn precision_of(path: &str) -> String {
    let p = path.to_lowercase();
    if p.ends_with(".gguf") {
        for q in ["q2_k", "q3_k", "q4_0", "q4_1", "q4_k", "q5_0", "q5_1", "q5_k", "q6_k", "q8_0", "iq4", "iq3", "f16", "bf16"] {
            if p.contains(q) {
                return q.to_uppercase();
            }
        }
        return "GGUF".into();
    }
    if p.contains("fp8") || p.contains("e4m3") || p.contains("e5m2") { "fp8" }
    else if p.contains("bf16") { "bf16" }
    else if p.contains("fp16") || p.contains("f16") { "fp16" }
    else if p.contains("fp32") || p.contains("f32") { "fp32" }
    else { "" }
    .into()
}

fn bytes_per_param(prec: &str) -> f64 {
    match prec {
        "fp8" | "Q8_0" => 1.0,
        "Q6_K" => 0.8,
        "Q5_0" | "Q5_1" | "Q5_K" => 0.7,
        "Q4_0" | "Q4_1" | "Q4_K" | "IQ4" => 0.56,
        "Q3_K" | "IQ3" => 0.45,
        "Q2_K" => 0.35,
        "fp32" => 4.0,
        _ => 2.0,
    }
}

/// Rank used to pick the best variant of the same file: lower is better.
fn precision_rank(prec: &str) -> u8 {
    match prec {
        "fp8" => 0,
        "Q8_0" => 1,
        "bf16" => 2,
        "fp16" | "F16" => 3,
        "Q6_K" | "Q5_0" | "Q5_1" | "Q5_K" => 4,
        "Q4_0" | "Q4_1" | "Q4_K" => 5,
        "fp32" => 7,
        "" => 6,
        _ => 8,
    }
}

/// File name with precision/quantisation words removed: files differing only in precision share a key.
fn group_key(dir: &str, path: &str) -> String {
    let mut k = path.to_lowercase();
    for t in ["_fp8_e4m3fn_scaled", "_fp8_scaled", "_fp8_e4m3fn", "_fp8_e5m2", "_fp8", "-fp8", "_bf16", "-bf16", "_fp16", "-fp16", "_fp32", "-fp32", ".safetensors", ".gguf", "_scaled"] {
        k = k.replace(t, "");
    }
    for q in ["q2_k", "q3_k_s", "q3_k_m", "q4_0", "q4_1", "q4_k_s", "q4_k_m", "q5_0", "q5_1", "q5_k_s", "q5_k_m", "q6_k", "q8_0"] {
        k = k.replace(&format!("-{q}"), "").replace(&format!("_{q}"), "").replace(&format!(".{q}"), "");
    }
    format!("{dir}/{k}")
}

struct Family {
    name: &'static str,
    keys: &'static [&'static str],
    kind: &'static str,
    modes: &'static [&'static str],
    quality: u8,
    speed: u8,
    /// parameter count (billions) the quality figure refers to
    ref_b: f64,
    typical: &'static [(&'static str, &'static str)],
    workflows: &'static [&'static str],
}

const FAMILIES: &[Family] = &[
    Family { name: "Wan 2.2", keys: &["wan2.2", "wan-2.2", "wan2_2", "wan22"], kind: "video", modes: &["t2v", "i2v"], quality: 5, speed: 2, ref_b: 14.0,
        typical: &[("size", "×16 (5B: ×32)"), ("frames", "4n+1 (81)"), ("fps", "16 (5B: 24)"), ("steps", "4 Lightning · 20 full")],
        workflows: &["wan22-t2v-lightning", "wan22-t2v-full", "wan22-i2v-lightning", "wan22-flf2v-lightning", "wan22-ti2v-5b"] },
    Family { name: "Wan 2.1", keys: &["wan2.1", "wan-2.1", "wan2_1", "wan21"], kind: "video", modes: &["t2v", "i2v"], quality: 4, speed: 2, ref_b: 14.0,
        typical: &[("size", "×16"), ("frames", "4n+1 (81)"), ("fps", "16"), ("steps", "20-30")], workflows: &[] },
    Family { name: "LTX-Video", keys: &["ltx-video", "ltxv", "ltx_video", "ltx-2", "ltx2"], kind: "video", modes: &["t2v", "i2v"], quality: 3, speed: 5, ref_b: 2.0,
        typical: &[("size", "×32"), ("frames", "8n+1 (97)"), ("fps", "24"), ("steps", "20-30")], workflows: &["ltxv-t2v", "ltxv-i2v", "ltx25-t2v", "ltx25-i2v"] },
    Family { name: "MiniMax H3", keys: &["minimax-h3", "minimax_h3", "minimaxh3"], kind: "video", modes: &["t2v", "i2v"], quality: 5, speed: 1, ref_b: 32.0,
        typical: &[("size", "×32 (864×480)"), ("frames", "17k+5 (124 = 5 s)"), ("fps", "24"), ("steps", "20 · 4-8 turbo LoRA")], workflows: &["minimax-h3-t2v", "minimax-h3-i2v"] },
    Family { name: "HunyuanVideo", keys: &["hunyuanvideo", "hunyuan-video", "hunyuan_video", "skyreels"], kind: "video", modes: &["t2v"], quality: 5, speed: 1, ref_b: 13.0,
        typical: &[("size", "×16"), ("frames", "4n+1 (65-129)"), ("fps", "24"), ("steps", "30")], workflows: &[] },
    Family { name: "CogVideoX", keys: &["cogvideo"], kind: "video", modes: &["t2v", "i2v"], quality: 3, speed: 2, ref_b: 5.0,
        typical: &[("size", "720×480"), ("frames", "49"), ("fps", "8"), ("steps", "50")], workflows: &[] },
    Family { name: "Mochi", keys: &["mochi"], kind: "video", modes: &["t2v"], quality: 4, speed: 1, ref_b: 10.0,
        typical: &[("size", "848×480"), ("frames", "6n+1 (31)"), ("fps", "30"), ("steps", "30-64")], workflows: &[] },
    Family { name: "Stable Video Diffusion", keys: &["stable-video-diffusion", "svd"], kind: "video", modes: &["i2v"], quality: 2, speed: 3, ref_b: 1.5,
        typical: &[("size", "1024×576"), ("frames", "14 / 25"), ("fps", "6-8"), ("steps", "25")], workflows: &[] },
    Family { name: "FLUX", keys: &["flux"], kind: "image", modes: &["t2i"], quality: 5, speed: 3, ref_b: 12.0,
        typical: &[("size", "1024×1024"), ("frames", "-"), ("fps", "-"), ("steps", "4 schnell · 20-28 dev")], workflows: &[] },
    Family { name: "SDXL", keys: &["stable-diffusion-xl", "sdxl"], kind: "image", modes: &["t2i"], quality: 3, speed: 4, ref_b: 3.5,
        typical: &[("size", "1024×1024"), ("frames", "-"), ("fps", "-"), ("steps", "25-30")], workflows: &[] },
];

fn is_commercial(license: &str) -> Option<bool> {
    let l = license.to_lowercase();
    if l.is_empty() || l == "?" { return None; }
    if l.contains("nc") || l.contains("non-commercial") || l.contains("research") { return Some(false); }
    if ["apache", "mit", "openrail", "bsd", "cc-by-4", "cc0", "unlicense"].iter().any(|a| l.contains(a)) { return Some(true); }
    None
}

fn vram_for(main_gb: f64) -> u32 {
    let need = main_gb * 1.2 + 3.0;
    *[8u32, 12, 16, 24, 32, 48, 80].iter().find(|v| (**v as f64) >= need).unwrap_or(&80)
}

pub fn analyze(id: &str, info: &Value, license: &str, files: &mut [RepoFile]) -> Insight {
    let hay = format!("{id} {}", info["tags"].as_array().map(|a| a.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default()).to_lowercase();
    let squash = |t: &str| t.chars().filter(|c| !"-_ ".contains(*c)).collect::<String>();
    let flat = squash(&hay);
    let fam = FAMILIES.iter().find(|f| f.keys.iter().any(|k| flat.contains(&squash(k))));
    let pipe = info["pipeline_tag"].as_str().unwrap_or("");

    // what it can do
    let mut modes: Vec<String> = vec![];
    fn push(v: &mut Vec<String>, m: &str) {
        if !v.iter().any(|x| x == m) {
            v.push(m.into());
        }
    }
    if pipe == "image-to-video" || hay.contains("image-to-video") || hay.contains("i2v") { push(&mut modes, "i2v"); }
    if pipe == "text-to-video" || hay.contains("text-to-video") || hay.contains("t2v") { push(&mut modes, "t2v"); }
    if pipe == "text-to-image" || hay.contains("text-to-image") { push(&mut modes, "t2i"); }
    if pipe == "image-to-image" { push(&mut modes, "i2i"); }
    if hay.contains("flf") || hay.contains("first-last") || hay.contains("keyframe") { push(&mut modes, "flf"); }
    if modes.is_empty() {
        if let Some(f) = fam { f.modes.iter().for_each(|m| push(&mut modes, m)); }
    }
    let kind = if modes.iter().any(|m| m == "t2i" || m == "i2i") && !modes.iter().any(|m| m == "t2v" || m == "i2v") { "image" }
        else if let Some(f) = fam { f.kind } else { "video" };

    // main weights (the big files that carry the model)
    let main: Vec<usize> = (0..files.len()).filter(|i| matches!(files[*i].dir.as_str(), "diffusion_models" | "checkpoints" | "unet")).collect();
    let biggest = main.iter().map(|i| files[*i].size).max().unwrap_or(0) as f64 / 1e9;
    let main_prec = main.iter().filter(|i| files[**i].size as f64 / 1e9 > biggest * 0.5).map(|i| files[*i].precision.clone()).find(|p| !p.is_empty()).unwrap_or_default();
    let declared = info["safetensors"]["total"].as_f64().unwrap_or(0.0) / 1e9;
    let params_b = if declared > 0.1 { declared } else if biggest > 0.0 { biggest / bytes_per_param(&main_prec) } else { 0.0 };

    // pick the best variant inside each group of look-alike files
    let usable = |d: &str| matches!(d, "diffusion_models" | "checkpoints" | "unet" | "vae" | "text_encoders");
    let mut best: std::collections::HashMap<String, (u8, usize)> = std::collections::HashMap::new();
    for (i, f) in files.iter().enumerate() {
        if !usable(&f.dir) { continue; }
        let k = group_key(&f.dir, &f.path);
        let r = precision_rank(&f.precision);
        if best.get(&k).map(|(br, _)| r < *br).unwrap_or(true) { best.insert(k, (r, i)); }
    }
    let picks: Vec<usize> = best.values().map(|(_, i)| *i).collect();
    for i in &picks { files[*i].recommended = true; }
    let total_gb: f64 = picks.iter().map(|i| files[*i].size as f64 / 1e9).sum();
    let too_many = picks.len() > 6 || total_gb > 45.0;
    let rec_main = picks.iter().filter(|i| matches!(files[**i].dir.as_str(), "diffusion_models" | "checkpoints" | "unet")).map(|i| files[*i].size).max().unwrap_or(0) as f64 / 1e9;

    // quality / speed estimates
    let name_l = id.to_lowercase();
    let mut quality = match fam {
        Some(f) => {
            let mut q = f.quality as i32;
            if params_b > 0.0 && params_b < f.ref_b * 0.5 { q -= 1; }
            q
        }
        None => if params_b >= 10.0 { 4 } else if params_b >= 3.0 { 3 } else if params_b >= 1.0 { 2 } else { 1 },
    };
    if matches!(main_prec.as_str(), "Q2_K" | "Q3_K" | "IQ3" | "Q4_0" | "Q4_1" | "Q4_K" | "IQ4") { quality -= 1; }
    let mut speed = match fam {
        Some(f) => f.speed as i32,
        None => if params_b >= 10.0 { 1 } else if params_b >= 4.0 { 2 } else if params_b >= 1.5 { 3 } else { 4 },
    };
    if ["lightning", "turbo", "distill", "schnell", "fast", "4step", "lcm"].iter().any(|w| name_l.contains(w)) { speed += 2; quality -= 0; }
    let typical = fam.map(|f| f.typical.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()).unwrap_or_default();
    let workflows = fam.map(|f| f.workflows.iter().map(|w| w.to_string()).collect()).unwrap_or_default();

    Insight {
        kind: kind.into(), modes, family: fam.map(|f| f.name.to_string()).unwrap_or_default(),
        params_b: (params_b * 10.0).round() / 10.0, precision: main_prec,
        vram_gb: if rec_main > 0.0 { vram_for(rec_main) } else { 0 },
        quality: quality.clamp(1, 5) as u8, speed: speed.clamp(1, 5) as u8,
        basis: if fam.is_some() { "family".into() } else { "size".into() },
        commercial: is_commercial(license), typical, workflows, too_many,
    }
}

pub async fn search(query: &str, video_only: bool) -> Result<Vec<Hit>, String> {
    let mut req = client()?.get(API).query(&[("search", query), ("sort", "downloads"), ("direction", "-1"), ("limit", "20")]);
    if video_only {
        req = req.query(&[("filter", "image-to-video")]);
    }
    let r = req.send().await.map_err(|e| format!("Could not connect to Hugging Face: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("HuggingFace: HTTP {}", r.status()));
    }
    let v: Value = r.json().await.map_err(|e| e.to_string())?;
    Ok(v.as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            Some(Hit {
                id: m["id"].as_str()?.to_string(),
                downloads: m["downloads"].as_u64().unwrap_or(0),
                likes: m["likes"].as_u64().unwrap_or(0),
                tag: m["pipeline_tag"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect())
}

pub async fn repo(id: &str) -> Result<Repo, String> {
    let c = client()?;
    let info = c.get(format!("{API}/{id}")).send().await.map_err(|e| format!("Could not connect to Hugging Face: {e}"))?;
    if info.status().as_u16() == 404 {
        return Err(format!("\"{id}\" was not found on Hugging Face. Check the name or the link."));
    }
    if !info.status().is_success() {
        return Err(format!("HuggingFace: HTTP {}", info.status()));
    }
    let info: Value = info.json().await.map_err(|e| e.to_string())?;
    let tree = c.get(format!("{API}/{id}/tree/main")).query(&[("recursive", "true")]).send().await.map_err(|e| e.to_string())?;
    if !tree.status().is_success() {
        return Err(format!("Hugging Face: HTTP {} (private or gated repo?)", tree.status()));
    }
    let tree: Value = tree.json().await.map_err(|e| e.to_string())?;
    let mut diffusers = false;
    let mut files = vec![];
    for f in tree.as_array().cloned().unwrap_or_default() {
        let path = f["path"].as_str().unwrap_or("").to_string();
        if path == "model_index.json" {
            diffusers = true;
        }
        let lower = path.to_lowercase();
        if f["type"] == "file" && WEIGHT_EXT.iter().any(|e| lower.ends_with(e)) {
            let size = f["lfs"]["size"].as_u64().or(f["size"].as_u64()).unwrap_or(0);
            files.push(RepoFile { dir: guess_dir(&path).into(), precision: precision_of(&path), recommended: false, path, size });
        }
    }
    let license = info["cardData"]["license"].as_str().unwrap_or("?").to_string();
    let insight = analyze(id, &info, &license, &mut files);
    Ok(Repo {
        id: id.into(),
        license,
        gated: info["gated"].as_bool().unwrap_or(false) || info["gated"].is_string(),
        diffusers,
        files,
        insight,
    })
}

/// Validates untrusted UI input and builds a models.json-shaped pack. The values end up inside a shell script
/// on the worker, so only a strict character set is accepted.
pub fn build_pack(repo: &str, name: &str, kind: &str, vram_gb: u32, license: &str, files: &[(String, String, u64)], insight: Value) -> Result<Value, String> {
    let safe = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c)) && !s.contains("..");
    if parse_repo(repo).as_deref() != Some(repo) || !safe(repo) {
        return Err("Invalid repo.".into());
    }
    if files.is_empty() {
        return Err("Pick at least one file.".into());
    }
    let mut out = vec![];
    let mut bytes = 0u64;
    for (path, dir, size) in files {
        if !DIRS.contains(&dir.as_str()) {
            return Err(format!("Unknown folder: {dir}"));
        }
        if !safe(path) || !WEIGHT_EXT.iter().any(|e| path.to_lowercase().ends_with(e)) {
            return Err(format!("Invalid file: {path}"));
        }
        bytes += size;
        out.push(json!({"dir": dir, "url": format!("https://huggingface.co/{repo}/resolve/main/{path}")}));
    }
    let clean = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(80).collect::<String>();
    let name = clean(name);
    let name = if name.is_empty() { repo.to_string() } else { name };
    let id = format!("hf-{}", repo.to_lowercase().replace('/', "--"));
    Ok(json!({
        "id": id, "name": name, "kind": if kind == "image" { "image" } else { "video" },
        "license": clean(license), "vram_gb": vram_gb.clamp(4, 80), "size_gb": ((bytes as f64) / 1e9 * 10.0).round() / 10.0,
        "desc_el": format!("Added from Hugging Face ({repo}). It needs a workflow before you can use it."),
        "desc_en": format!("Added from HuggingFace ({repo}). It needs a workflow to be used."),
        "in_template": false, "custom": true, "repo": repo, "insight": insight, "files": out
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_links_and_names() {
        assert_eq!(parse_repo("Comfy-Org/Wan_2.2_ComfyUI_Repackaged").as_deref(), Some("Comfy-Org/Wan_2.2_ComfyUI_Repackaged"));
        assert_eq!(parse_repo("https://huggingface.co/Lightricks/LTX-Video/tree/main").as_deref(), Some("Lightricks/LTX-Video"));
        assert_eq!(
            parse_repo("https://huggingface.co/a/b/resolve/main/x/y.safetensors").as_deref(),
            Some("a/b")
        );
        assert_eq!(parse_repo("wan image to video"), None);
        assert_eq!(parse_repo("ltx"), None);
    }

    #[test]
    fn guesses_folders() {
        assert_eq!(guess_dir("split_files/vae/wan_2.1_vae.safetensors"), "vae");
        assert_eq!(guess_dir("split_files/text_encoders/umt5_xxl_fp8.safetensors"), "text_encoders");
        assert_eq!(guess_dir("split_files/diffusion_models/wan2.2_i2v_high.safetensors"), "diffusion_models");
        assert_eq!(guess_dir("sd_xl_base_1.0.safetensors"), "checkpoints");
    }

    fn file(path: &str, gb: f64) -> RepoFile {
        RepoFile { dir: guess_dir(path).into(), precision: precision_of(path), recommended: false, path: path.into(), size: (gb * 1e9) as u64 }
    }

    #[test]
    fn analysis_recognises_wan_and_prefers_fp8() {
        let mut files = vec![
            file("split_files/diffusion_models/wan2.2_i2v_high_noise_14B_fp8_scaled.safetensors", 14.3),
            file("split_files/diffusion_models/wan2.2_i2v_high_noise_14B_fp16.safetensors", 28.6),
            file("split_files/vae/wan_2.1_vae.safetensors", 0.25),
            file("split_files/text_encoders/umt5_xxl_fp8_e4m3fn_scaled.safetensors", 6.7),
        ];
        let info = json!({"pipeline_tag": "image-to-video", "tags": ["comfyui"]});
        let ins = analyze("Comfy-Org/Wan_2.2_ComfyUI_Repackaged", &info, "apache-2.0", &mut files);
        assert_eq!(ins.family, "Wan 2.2");
        assert_eq!(ins.kind, "video");
        assert!(ins.modes.contains(&"i2v".to_string()));
        assert_eq!(ins.vram_gb, 24);
        assert_eq!(ins.commercial, Some(true));
        assert!(ins.workflows.contains(&"wan22-i2v-lightning".to_string()));
        assert!(files[0].recommended && !files[1].recommended, "fp8 should win over fp16");
        assert!(ins.quality >= 4 && ins.speed <= 3);
    }

    #[test]
    fn analysis_of_unknown_small_gguf_model() {
        let mut files = vec![file("model-Q4_K_M.gguf", 3.0)];
        let ins = analyze("someone/mystery-video", &json!({"tags": ["text-to-video"]}), "cc-by-nc-4.0", &mut files);
        assert_eq!(ins.family, "");
        assert_eq!(ins.basis, "size");
        assert_eq!(ins.commercial, Some(false));
        assert_eq!(files[0].precision, "Q4_K");
        assert!(ins.quality <= 2);
    }

    #[test]
    fn pack_rejects_shell_tricks() {
        let ok = vec![("a/b.safetensors".to_string(), "vae".to_string(), 2_000_000_000)];
        let p = build_pack("org/repo", "My\nmodel", "video", 24, "mit", &ok, Value::Null).unwrap();
        assert_eq!(p["name"], "Mymodel");
        assert_eq!(p["size_gb"], 2.0);
        assert!(build_pack("org/repo", "x", "video", 24, "", &[("a\";rm -rf /;\".safetensors".into(), "vae".into(), 1)], Value::Null).is_err());
        assert!(build_pack("org/repo", "x", "video", 24, "", &[("a.safetensors".into(), "../etc".into(), 1)], Value::Null).is_err());
        assert!(build_pack("org/re po", "x", "video", 24, "", &ok, Value::Null).is_err());
    }
}
