use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InputDef {
    pub key: String,
    /// text | int | float | seed | image (base64 JPEG)
    #[serde(rename = "type", default)]
    pub ty: String,
    #[serde(default)]
    pub default: Value,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub advanced: bool,
    #[serde(default)]
    pub label_el: String,
    #[serde(default)]
    pub label_en: String,
    /// Fixed choices (shown as a drop-down), e.g. languages or voices.
    #[serde(default)]
    pub options: Vec<String>,
}

fn prio() -> u32 {
    100
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Meta {
    pub id: String,
    #[serde(default)]
    pub name_el: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub desc_el: String,
    #[serde(default)]
    pub desc_en: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub vram_gb: u32,
    #[serde(default)]
    pub est_sec: u32,
    /// Model pack (models.json id) this workflow needs installed on the workers.
    #[serde(default)]
    pub pack: String,
    /// Lower comes first in the mode list (the first text mode is the default).
    #[serde(default = "prio")]
    pub priority: u32,
    #[serde(default)]
    pub inputs: Vec<InputDef>,
    /// Hosted API modes (fal, replicate): the provider, its model id, the request template and a typical price.
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub request: Value,
    #[serde(default)]
    pub est_usd: f64,
    /// "open" (runs on this PC or a rented GPU) or "paid" (per result, needs an API key). Empty = derived from provider.
    #[serde(default)]
    pub license: String,
    /// What the model is best at, shown in the mode list and offered to the chat / MCP for recommendations.
    #[serde(default)]
    pub good_en: String,
    #[serde(default)]
    pub good_el: String,
    /// Languages the model can speak or sing, for sound modes (shown in the UI, offered to the chat / MCP).
    #[serde(default)]
    pub languages: Vec<String>,
    /// Safety / licence note shown before use (voice cloning, faces).
    #[serde(default)]
    pub notice_en: String,
    #[serde(default)]
    pub notice_el: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Workflow {
    pub meta: Meta,
    pub workflow: Value,
    /// "bundled" | "user"
    pub source: String,
    pub file: String,
}

/// A user-facing parameter found inside a plain ComfyUI export, with the node inputs it controls.
pub struct Detected {
    pub def: InputDef,
    pub targets: Vec<(String, String)>,
}

fn num_in(node: &Value, k: &str) -> Option<f64> {
    node["inputs"].get(k).and_then(|v| v.as_f64())
}

/// Reads a plain ComfyUI API export and finds the settings a user would want to change
/// (size, frames, fps, seed, steps, cfg, negative prompt) together with their current values as defaults.
pub fn detect(workflow: &Value) -> Vec<Detected> {
    let Some(obj) = workflow.as_object() else { return vec![] };
    let mut ids: Vec<&String> = obj.keys().collect();
    ids.sort_by_key(|k| k.parse::<u64>().unwrap_or(u64::MAX));
    let class = |id: &String| obj[id]["class_type"].as_str().unwrap_or("");
    let mut out: Vec<Detected> = vec![];
    let mut add = |key: &str, ty: &str, default: Value, min: f64, max: f64, step: f64, advanced: bool, el: &str, en: &str, targets: Vec<(String, String)>| {
        if !targets.is_empty() {
            out.push(Detected {
                def: InputDef {
                    key: key.into(), ty: ty.into(), default, min: Some(min), max: Some(max), step: Some(step),
                    advanced, label_el: el.into(), label_en: en.into(), ..Default::default()
                },
                targets,
            });
        }
    };

    // size / frames live on the latent or conditioning node that creates the video
    let sized: Vec<&String> = ids.iter().copied().filter(|id| {
        let c = class(id);
        (c.contains("Latent") || c.contains("ToVideo") || c.starts_with("Empty")) && num_in(&obj[*id], "width").is_some() && num_in(&obj[*id], "height").is_some()
    }).collect();
    if let Some(first) = sized.first() {
        let n = &obj[*first];
        add("width", "int", json!(num_in(n, "width").unwrap()), 64.0, 2048.0, 16.0, false, "Πλάτος", "Width", sized.iter().map(|i| ((*i).clone(), "width".to_string())).collect());
        add("height", "int", json!(num_in(n, "height").unwrap()), 64.0, 2048.0, 16.0, false, "Ύψος", "Height", sized.iter().map(|i| ((*i).clone(), "height".to_string())).collect());
        for lk in ["length", "frames", "num_frames", "video_frames"] {
            let t: Vec<_> = sized.iter().filter(|i| num_in(&obj[**i], lk).is_some()).map(|i| ((*i).clone(), lk.to_string())).collect();
            if let Some((id, _)) = t.first() {
                add("length", "int", json!(num_in(&obj[id], lk).unwrap()), 1.0, 257.0, 1.0, false, "Καρέ", "Frames", t);
                break;
            }
        }
    }
    let fps: Vec<_> = ids.iter().filter(|i| class(i).contains("Video") || class(i).contains("Conditioning")).flat_map(|i| {
        ["fps", "frame_rate"].iter().filter(|k| num_in(&obj[*i], **k).is_some()).map(|k| ((*i).clone(), k.to_string())).collect::<Vec<_>>()
    }).collect();
    if let Some((id, k)) = fps.first() {
        add("fps", "int", json!(num_in(&obj[id], k).unwrap().round()), 1.0, 60.0, 1.0, false, "FPS", "FPS", fps.clone());
    }
    let seeds: Vec<_> = ids.iter().flat_map(|i| {
        ["seed", "noise_seed"].iter().filter(|k| num_in(&obj[*i], **k).is_some()).map(|k| ((*i).clone(), k.to_string())).collect::<Vec<_>>()
    }).collect();
    add("seed", "seed", json!("__RANDOM_INT__"), 0.0, 1e15, 1.0, false, "Seed (κενό = τυχαίο)", "Seed (blank = random)", seeds);

    // steps / cfg only when there is a single plain sampler (split-step pairs would break)
    let advanced_pair = ids.iter().any(|i| class(i) == "KSamplerAdvanced");
    if !advanced_pair {
        let st: Vec<_> = ids.iter().filter(|i| matches!(class(i), "KSampler" | "BasicScheduler" | "LTXVScheduler") && num_in(&obj[**i], "steps").is_some()).map(|i| ((*i).clone(), "steps".to_string())).collect();
        if st.len() == 1 {
            add("steps", "int", json!(num_in(&obj[&st[0].0], "steps").unwrap()), 1.0, 100.0, 1.0, true, "Βήματα (steps)", "Steps", st);
        }
        let cf: Vec<_> = ids.iter().filter(|i| matches!(class(i), "KSampler" | "CFGGuider") && num_in(&obj[**i], "cfg").is_some()).map(|i| ((*i).clone(), "cfg".to_string())).collect();
        if cf.len() == 1 {
            add("cfg", "float", json!(num_in(&obj[&cf[0].0], "cfg").unwrap()), 0.0, 30.0, 0.1, true, "CFG", "CFG", cf);
        }
    }
    let neg: Vec<_> = ids.iter().filter(|i| {
        class(i) == "CLIPTextEncode" && obj[**i]["_meta"]["title"].as_str().unwrap_or("").to_lowercase().contains("negative") && obj[**i]["inputs"]["text"].is_string()
    }).map(|i| ((*i).clone(), "text".to_string())).collect();
    if let Some((id, _)) = neg.first() {
        let d = obj[id]["inputs"]["text"].clone();
        add("negative", "text", d, 0.0, 0.0, 0.0, true, "Αρνητικό prompt", "Negative prompt", neg.clone());
    }
    out
}

fn image_input() -> InputDef {
    InputDef {
        key: "image".into(),
        ty: "image".into(),
        required: true,
        label_el: "Εικόνα εκκίνησης".into(),
        label_en: "Start image".into(),
        ..Default::default()
    }
}

pub fn user_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("workflows")
}

fn parse_file(path: &Path, source: &str) -> Result<Workflow, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let file = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

    let is_plain = !(v.get("meta").is_some() && v.get("workflow").is_some());
    let (mut meta, workflow) = if !is_plain {
        let m: Meta = serde_json::from_value(v["meta"].clone()).map_err(|e| format!("meta: {e}"))?;
        (m, v["workflow"].clone())
    } else {
        // Plain ComfyUI API export: infer a prompt-only mode.
        (Meta::default(), v)
    };
    if !workflow.is_object() {
        return Err("workflow must be a JSON object in ComfyUI API format".into());
    }
    if meta.id.is_empty() {
        meta.id = stem.clone();
    }
    if meta.name_el.is_empty() {
        meta.name_el = stem.clone();
    }
    if meta.name_en.is_empty() {
        meta.name_en = meta.name_el.clone();
    }
    let has_load_image = workflow.as_object().map(|o| o.values().any(|n| n["class_type"] == "LoadImage")).unwrap_or(false);
    if meta.kind.is_empty() {
        meta.kind = if has_load_image { "i2v" } else { "t2v" }.into();
    }
    if meta.inputs.is_empty() {
        meta.inputs.push(InputDef {
            key: "prompt".into(),
            ty: "text".into(),
            required: true,
            label_el: "Prompt".into(),
            label_en: "Prompt".into(),
            ..Default::default()
        });
        if has_load_image {
            meta.inputs.push(image_input());
        }
        if is_plain {
            meta.inputs.extend(detect(&workflow).into_iter().map(|d| d.def));
        }
    }
    Ok(Workflow { meta, workflow, source: source.into(), file })
}

/// Returns (workflows, errors). User files with the same id override bundled ones.
pub fn load_all(bundled: &Path, user: &Path) -> (Vec<Workflow>, Vec<String>) {
    let mut out: Vec<Workflow> = vec![];
    let mut errs = vec![];
    for (dir, src) in [(bundled, "bundled"), (user, "user")] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for p in files {
            if p.extension().map(|e| e.eq_ignore_ascii_case("json")).unwrap_or(false) {
                match parse_file(&p, src) {
                    Ok(w) => {
                        // a shared file must not silently replace one of the app's own modes
                        if src == "user" && out.iter().any(|x| x.source == "bundled" && x.meta.id == w.meta.id) {
                            errs.push(format!("{}: the id \"{}\" belongs to a built-in mode; change \"id\" in the file", p.display(), w.meta.id));
                            continue;
                        }
                        out.retain(|x| x.meta.id != w.meta.id);
                        out.push(w);
                    }
                    Err(e) => errs.push(format!("{}: {e}", p.display())),
                }
            }
        }
    }
    // Text modes first (default tab), then fastest (cheapest) first; modes without an estimate go last.
    out.sort_by_key(|w| (w.meta.inputs.iter().any(|i| i.ty == "image"), w.meta.priority, w.meta.est_sec == 0, w.meta.est_sec));
    (out, errs)
}

/// The RunPod twin of an open-source mode (`wan22-t2v-lightning` -> `runpod-wan22-t2v-lightning`), when `provider` is
/// "runpod" and such a mode exists. Otherwise the id is returned unchanged (hosted modes, tools that only run on this PC).
pub fn effective_id(wfs: &[Workflow], id: &str, provider: &str) -> String {
    if provider == "runpod" && wfs.iter().any(|w| w.meta.id == id && w.meta.provider.is_empty()) {
        let twin = format!("runpod-{id}");
        if wfs.iter().any(|w| w.meta.id == twin && w.meta.provider == "runpod") {
            return twin;
        }
    }
    id.to_string()
}

/// The job's parameters as typed values (defaults filled in, random seed resolved) for hosted request templates.
pub fn hosted_params(wf: &Workflow, params: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for d in &wf.meta.inputs {
        let mut v = typed(d, params.get(&d.key));
        if v == json!("__RANDOM_INT__") {
            v = json!(uuid::Uuid::new_v4().as_u128() as u32 as i64);
        }
        out.insert(d.key.clone(), v);
    }
    out
}

fn typed(def: &InputDef, v: Option<&Value>) -> Value {
    let raw = v.cloned().unwrap_or_else(|| def.default.clone());
    match def.ty.as_str() {
        "int" | "float" => {
            let mut n = match &raw {
                Value::Number(n) => n.as_f64().unwrap_or(0.0),
                Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
                _ => 0.0,
            };
            if let Some(m) = def.min { n = n.max(m); }
            if let Some(m) = def.max { n = n.min(m); }
            if def.ty == "int" { json!(n.round() as i64) } else { json!(n) }
        }
        "image" | "audio" | "video" => match raw {
            Value::String(_) => raw,
            _ => json!(""),
        },
        "seed" => match &raw {
            Value::Number(_) => raw,
            Value::String(s) => s.trim().parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!("__RANDOM_INT__")),
            _ => json!("__RANDOM_INT__"),
        },
        _ => match raw {
            Value::String(_) => raw,
            Value::Null => json!(""),
            other => json!(other.to_string()),
        },
    }
}

/// Substitutes `{{key}}` placeholders. A string that is exactly one placeholder keeps the value's type.
pub fn fill(wf: &Workflow, params: &Map<String, Value>) -> Value {
    let values: Vec<(String, Value)> = wf
        .meta
        .inputs
        .iter()
        .map(|d| (d.key.clone(), typed(d, params.get(&d.key))))
        .collect();
    let mut w = wf.workflow.clone();
    walk(&mut w, &values);

    // Plain exports without placeholders: write the prompt into the positive text-encoder.
    let has_placeholders = wf.workflow.to_string().contains("{{prompt}}");
    if !has_placeholders {
        if let (Some(obj), Some(Value::String(p))) = (w.as_object_mut(), params.get("prompt")) {
            let mut best: Option<String> = None;
            for (id, node) in obj.iter() {
                if node["class_type"] == "CLIPTextEncode" {
                    let title = node["_meta"]["title"].as_str().unwrap_or("").to_lowercase();
                    if title.contains("negative") { continue; }
                    if title.contains("positive") { best = Some(id.clone()); break; }
                    if best.is_none() { best = Some(id.clone()); }
                }
            }
            if let Some(id) = best {
                obj[&id]["inputs"]["text"] = json!(p);
            }
        }
    }
    // Plain exports: write the values of the auto-detected settings into the nodes they came from.
    if !wf.workflow.to_string().contains("{{") {
        let defs: Vec<&InputDef> = wf.meta.inputs.iter().collect();
        for d in detect(&wf.workflow) {
            let Some(def) = defs.iter().find(|x| x.key == d.def.key) else { continue };
            let Some(v) = params.get(&d.def.key) else { continue };
            let val = typed(def, Some(v));
            for (node, input) in &d.targets {
                if let Some(slot) = w.get_mut(node).and_then(|n| n["inputs"].get_mut(input)) {
                    *slot = val.clone();
                }
            }
        }
    }
    // Plain exports without an {{image}} placeholder: swap ComfyUI's LoadImage (needs a file on the worker)
    // for our inline node, which has the same IMAGE/MASK outputs.
    if let (Some(obj), Some(Value::String(img))) = (w.as_object_mut(), params.get("image")) {
        if !img.is_empty() && !wf.workflow.to_string().contains("{{image}}") {
            for node in obj.values_mut().filter(|n| n["class_type"] == "LoadImage") {
                node["class_type"] = json!("AIVSLoadImageBase64");
                node["inputs"] = json!({ "image_b64": img });
            }
        }
    }
    w
}

fn walk(v: &mut Value, values: &[(String, Value)]) {
    match v {
        Value::String(s) => {
            if let Some((_, val)) = values.iter().find(|(k, _)| *s == format!("{{{{{k}}}}}")) {
                *v = val.clone();
                return;
            }
            if s.contains("{{") {
                let mut t = s.clone();
                for (k, val) in values {
                    let rep = match val { Value::String(x) => x.clone(), o => o.to_string() };
                    t = t.replace(&format!("{{{{{k}}}}}"), &rep);
                }
                *s = t;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| walk(x, values)),
        Value::Object(o) => o.values_mut().for_each(|x| walk(x, values)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn open_modes_map_to_their_runpod_twin() {
        let mk = |id: &str, provider: &str| Workflow { meta: Meta { id: id.into(), provider: provider.into(), ..Default::default() }, workflow: Value::Null, source: "bundled".into(), file: String::new() };
        let wfs = vec![mk("wan22-t2v-lightning", ""), mk("runpod-wan22-t2v-lightning", "runpod"), mk("kokoro-tts", ""), mk("fal-flux-schnell", "fal")];
        assert_eq!(effective_id(&wfs, "wan22-t2v-lightning", "runpod"), "runpod-wan22-t2v-lightning");
        assert_eq!(effective_id(&wfs, "wan22-t2v-lightning", "local"), "wan22-t2v-lightning");
        assert_eq!(effective_id(&wfs, "kokoro-tts", "runpod"), "kokoro-tts");
        assert_eq!(effective_id(&wfs, "fal-flux-schnell", "runpod"), "fal-flux-schnell");
        assert_eq!(effective_id(&wfs, "unknown", "runpod"), "unknown");
    }

    use super::*;

    fn wf(path: &str) -> Workflow {
        parse_file(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workflows").join(path), "bundled").unwrap()
    }

    #[test]
    fn fill_keeps_types_and_clamps() {
        let w = wf("wan22-t2v-lightning.json");
        let mut p = Map::new();
        p.insert("prompt".into(), json!("a cat"));
        p.insert("width".into(), json!("99999"));
        p.insert("seed".into(), json!(""));
        let f = fill(&w, &p);
        assert_eq!(f["99"]["inputs"]["text"], "a cat");
        assert_eq!(f["104"]["inputs"]["width"], 1280);
        assert_eq!(f["104"]["inputs"]["length"], 81);
        assert_eq!(f["96"]["inputs"]["noise_seed"], "__RANDOM_INT__");
        assert!(!f.to_string().contains("{{"));
    }

    #[test]
    fn media_tools_are_wired_and_fill_cleanly() {
        for f in ["chatterbox-tts.json", "chatterbox-clone.json", "chatterbox-vc.json", "kokoro-tts.json", "rmbg-birefnet.json", "upscale-realesrgan.json", "qwen-image-edit.json", "interpolate-rife.json", "lipsync-latentsync.json", "whisper-subtitles.json", "whisper-subtitles-audio.json", "cutout-sam2.json", "erase-object.json", "face-restore-gfpgan.json", "colorize-ddcolor.json", "depth-da3.json", "trellis2-image-to-3d.json"] {
            let w = wf(f);
            let graph = w.workflow.to_string();
            // every input is used by the graph (prompt may be only a label for pure tools), every placeholder has an input
            for i in w.meta.inputs.iter().filter(|i| i.key != "prompt") {
                assert!(graph.contains(&format!("{{{{{}}}}}", i.key)), "{f}: input {} is not used", i.key);
            }
            let mut p = Map::new();
            for i in &w.meta.inputs {
                p.insert(i.key.clone(), match i.ty.as_str() { "image" | "audio" | "video" => json!("QUJD"), _ => i.default.clone() });
            }
            p.insert("prompt".into(), json!("hello"));
            let filled = fill(&w, &p).to_string();
            assert!(!filled.contains("{{"), "{f}: unfilled placeholder: {filled}");
        }
    }

    #[test]
    fn i2v_bundled_workflows_take_the_image() {
        for f in ["wan22-i2v-lightning.json", "wan22-ti2v-5b.json", "ltxv-i2v.json", "ltx25-i2v.json", "minimax-h3-i2v.json"] {
            let w = wf(f);
            assert!(w.meta.inputs.iter().any(|i| i.ty == "image"), "{f} has no image input");
            let mut p = Map::new();
            p.insert("prompt".into(), json!("a cat"));
            p.insert("image".into(), json!("QUJD"));
            let filled = fill(&w, &p);
            assert!(filled.to_string().contains("AIVSLoadImageBase64"), "{f}");
            assert!(filled.to_string().contains("QUJD"), "{f}");
            assert!(!filled.to_string().contains("{{"), "{f}");
        }
    }

    #[test]
    fn sound_video_workflows_fill_cleanly_and_end_in_createvideo_with_audio() {
        for f in ["ltx25-t2v.json", "ltx25-i2v.json", "minimax-h3-t2v.json", "minimax-h3-i2v.json"] {
            let w = wf(f);
            let graph = w.workflow.to_string();
            for i in w.meta.inputs.iter().filter(|i| i.key != "prompt") {
                assert!(graph.contains(&format!("{{{{{}}}}}", i.key)), "{f}: input {} is not used", i.key);
            }
            let mut p = Map::new();
            for i in &w.meta.inputs {
                p.insert(i.key.clone(), match i.ty.as_str() { "image" => json!("QUJD"), _ => i.default.clone() });
            }
            let filled = fill(&w, &p);
            assert!(!filled.to_string().contains("{{"), "{f}: unfilled placeholder");
            let obj = filled.as_object().unwrap();
            // the app swaps SaveVideo for the inline save node via CreateVideo, so both must exist and CreateVideo must carry the audio
            assert_eq!(obj.values().filter(|n| n["class_type"] == "SaveVideo").count(), 1, "{f}");
            let cv: Vec<_> = obj.values().filter(|n| n["class_type"] == "CreateVideo").collect();
            assert_eq!(cv.len(), 1, "{f}");
            assert!(cv[0]["inputs"].get("audio").is_some(), "{f}: no audio into CreateVideo");
            assert_eq!(w.meta.kind, if f.contains("i2v") { "i2v" } else { "t2v" });
        }
    }

    #[test]
    fn first_last_frame_workflow_takes_two_images() {
        let w = wf("wan22-flf2v-lightning.json");
        let keys: Vec<_> = w.meta.inputs.iter().filter(|i| i.ty == "image").map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["image", "image_end"]);
        let mut p = Map::new();
        p.insert("prompt".into(), json!("x"));
        p.insert("image".into(), json!("U1RBUlQ="));
        p.insert("image_end".into(), json!("RU5E"));
        let f = fill(&w, &p).to_string();
        assert!(f.contains("U1RBUlQ=") && f.contains("RU5E") && !f.contains("{{"));
        assert!(f.contains("WanFirstLastFrameToVideo"));
    }

    #[test]
    fn text_modes_sort_before_image_modes_and_wan_is_default() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workflows");
        let (all, errs) = load_all(&dir, &dir.join("none"));
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(all[0].meta.id, "wan22-t2v-lightning");
        let first_img = all.iter().position(|w| w.meta.inputs.iter().any(|i| i.ty == "image")).unwrap();
        assert!(all[..first_img].iter().all(|w| !w.meta.inputs.iter().any(|i| i.ty == "image")));
        assert_eq!(all[first_img].meta.id, "wan22-i2v-lightning");
    }

    #[test]
    fn plain_export_with_load_image_becomes_i2v() {
        let dir = std::env::temp_dir().join("aivs_plain_i2v.json");
        std::fs::write(&dir, r#"{"1":{"class_type":"LoadImage","inputs":{"image":"a.png"}},"2":{"class_type":"CLIPTextEncode","_meta":{"title":"Positive"},"inputs":{"text":"x"}}}"#).unwrap();
        let w = parse_file(&dir, "user").unwrap();
        assert_eq!(w.meta.kind, "i2v");
        assert!(w.meta.inputs.iter().any(|i| i.ty == "image"));
        let mut p = Map::new();
        p.insert("prompt".into(), json!("new"));
        p.insert("image".into(), json!("QUJD"));
        let f = fill(&w, &p);
        assert_eq!(f["1"]["class_type"], "AIVSLoadImageBase64");
        assert_eq!(f["1"]["inputs"]["image_b64"], "QUJD");
        assert_eq!(f["2"]["inputs"]["text"], "new");
    }

    #[test]
    fn plain_export_parameters_are_found_and_applied() {
        let path = std::env::temp_dir().join("aivs_detect.json");
        std::fs::write(&path, r#"{
          "3":{"class_type":"KSampler","inputs":{"seed":5,"steps":25,"cfg":6.5,"model":["1",0],"positive":["6",0],"negative":["7",0],"latent_image":["5",0]}},
          "5":{"class_type":"EmptyHunyuanLatentVideo","inputs":{"width":960,"height":544,"length":65,"batch_size":1}},
          "6":{"class_type":"CLIPTextEncode","_meta":{"title":"Positive"},"inputs":{"text":"p"}},
          "7":{"class_type":"CLIPTextEncode","_meta":{"title":"Negative Prompt"},"inputs":{"text":"ugly"}},
          "9":{"class_type":"CreateVideo","inputs":{"fps":24,"images":["8",0]}}}"#).unwrap();
        let w = parse_file(&path, "user").unwrap();
        let keys: Vec<_> = w.meta.inputs.iter().map(|i| i.key.as_str()).collect();
        for k in ["prompt", "width", "height", "length", "fps", "seed", "steps", "cfg", "negative"] {
            assert!(keys.contains(&k), "missing {k} in {keys:?}");
        }
        assert_eq!(w.meta.inputs.iter().find(|i| i.key == "width").unwrap().default, json!(960.0));
        let mut p = Map::new();
        p.insert("prompt".into(), json!("new"));
        p.insert("width".into(), json!(704));
        p.insert("length".into(), json!(33));
        p.insert("steps".into(), json!(12));
        p.insert("negative".into(), json!("blur"));
        let f = fill(&w, &p);
        assert_eq!(f["5"]["inputs"]["width"], 704);
        assert_eq!(f["5"]["inputs"]["length"], 33);
        assert_eq!(f["5"]["inputs"]["height"], 544);
        assert_eq!(f["3"]["inputs"]["steps"], 12);
        assert_eq!(f["7"]["inputs"]["text"], "blur");
        assert_eq!(f["6"]["inputs"]["text"], "new");
    }

    #[test]
    fn plain_comfy_export_gets_prompt() {
        let w = Workflow {
            meta: Meta { id: "x".into(), inputs: vec![InputDef { key: "prompt".into(), ty: "text".into(), ..Default::default() }], ..Default::default() },
            workflow: json!({"1": {"class_type":"CLIPTextEncode","_meta":{"title":"Negative"},"inputs":{"text":"bad"}},
                             "2": {"class_type":"CLIPTextEncode","_meta":{"title":"Positive"},"inputs":{"text":"old"}}}),
            source: "user".into(), file: "x.json".into(),
        };
        let mut p = Map::new();
        p.insert("prompt".into(), json!("new"));
        let f = fill(&w, &p);
        assert_eq!(f["2"]["inputs"]["text"], "new");
        assert_eq!(f["1"]["inputs"]["text"], "bad");
    }
}
