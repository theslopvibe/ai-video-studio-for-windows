//! Hosted model APIs (fal.ai, Replicate): no GPU rental and no ComfyUI, you pay per result.
//! A hosted mode is a workflow JSON whose meta has `provider` and a `request` template; strings like "{width}" in the
//! template are replaced by the job's parameters. Model ids change often, so users can edit these files.
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PROVIDERS: &[(&str, &str)] = &[("fal", "fal_key"), ("replicate", "replicate_token"), ("openai", "openai_api_key"), ("gemini", "gemini_api_key"), ("runpod", "runpod_api_key")];

pub fn secret_name(provider: &str) -> Option<&'static str> {
    PROVIDERS.iter().find(|(p, _)| *p == provider).map(|(_, s)| *s)
}

/// Replaces "{key}" strings in `tpl` with values from `params`. A string that is exactly "{key}" keeps the value's type
/// (numbers stay numbers); a missing key removes the field so the model's default applies.
pub fn render(tpl: &Value, params: &Map<String, Value>) -> Value {
    match tpl {
        Value::String(s) => {
            if let Some(k) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
                return params.get(k).cloned().unwrap_or(Value::Null);
            }
            let mut out = s.clone();
            for (k, v) in params {
                let pat = format!("{{{k}}}");
                if out.contains(&pat) {
                    out = out.replace(&pat, &v.as_str().map(String::from).unwrap_or_else(|| v.to_string()));
                }
            }
            Value::String(out)
        }
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), render(v, params))).filter(|(_, v)| !v.is_null()).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|v| render(v, params)).collect()),
        other => other.clone(),
    }
}

/// First base64 image in a response: OpenAI `b64_json`, Gemini `inlineData.data` / `inline_data.data` / `output_image.data`
/// or any `data` string sitting next to an `image/*` mime type.
pub fn find_b64_image(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => {
            if let Some(b) = m.get("b64_json").and_then(|x| x.as_str()) {
                return Some(b.to_string());
            }
            let mime = ["mime_type", "mimeType"].iter().find_map(|k| m.get(*k).and_then(|x| x.as_str())).unwrap_or("");
            if mime.starts_with("image/") || m.contains_key("output_image") {
                if let Some(d) = m.get("data").and_then(|x| x.as_str()) {
                    return Some(d.to_string());
                }
            }
            m.values().find_map(find_b64_image)
        }
        Value::Array(a) => a.iter().find_map(find_b64_image),
        _ => None,
    }
}

fn decode_image(b64: &str) -> Result<(Vec<u8>, String), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|e| format!("bad image data: {e}"))?;
    let ext = if bytes.starts_with(&[0xFF, 0xD8]) { "jpg" } else if bytes.starts_with(b"RIFF") { "webp" } else { "png" };
    Ok((bytes, ext.into()))
}

/// First http(s) URL in the response that looks like a media file (video or image).
pub fn find_media_url(v: &Value) -> Option<String> {
    const EXT: &[&str] = &[".mp4", ".webm", ".mov", ".png", ".jpg", ".jpeg", ".webp", ".gif"];
    match v {
        Value::String(s) if s.starts_with("http") => {
            let path = s.split('?').next().unwrap_or("").to_lowercase();
            EXT.iter().any(|e| path.ends_with(e)).then(|| s.clone())
        }
        Value::Object(m) => {
            // prefer video over image when both exist
            for k in ["video", "videos"] {
                if let Some(u) = m.get(k).and_then(find_media_url) {
                    return Some(u);
                }
            }
            m.values().find_map(find_media_url)
        }
        Value::Array(a) => a.iter().find_map(find_media_url),
        _ => None,
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().user_agent("AI-Video-Studio").timeout(Duration::from_secs(120)).build().expect("http client")
}

async fn http_json(r: reqwest::RequestBuilder, what: &str) -> Result<Value, String> {
    let resp = r.send().await.map_err(|e| format!("{what}: {e}"))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = body["detail"].as_str().map(String::from).or_else(|| body["detail"].as_array().map(|a| a.iter().map(|x| x["msg"].as_str().unwrap_or("")).collect::<Vec<_>>().join("; "))).or_else(|| body["error"].as_str().map(String::from)).unwrap_or_else(|| body.to_string());
        return Err(match status.as_u16() {
            401 | 403 => format!("{what}: the API key was rejected. Check it in Settings. ({msg})"),
            402 => format!("{what}: not enough credit on your account. ({msg})"),
            404 => format!("{what}: model not found, the model id may have changed. Edit the mode's JSON. ({msg})"),
            _ => format!("{what}: HTTP {status} {msg}"),
        });
    }
    Ok(body)
}

async fn download(url: &str) -> Result<(Vec<u8>, String), String> {
    let r = client().get(url).timeout(Duration::from_secs(600)).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("download failed: HTTP {}", r.status()));
    }
    let ext = url.split('?').next().unwrap_or("").rsplit('.').next().unwrap_or("mp4").to_lowercase();
    Ok((r.bytes().await.map_err(|e| e.to_string())?.to_vec(), ext))
}

async fn wait<F: Fn(u64) + ?Sized>(cancel: &AtomicBool, tick: &F, t0: Instant, secs: u64) -> Result<(), String> {
    for _ in 0..(secs * 2) {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        tick(t0.elapsed().as_secs());
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}

pub async fn run(provider: &str, model: &str, key: &str, body: &Value, cancel: &AtomicBool, tick: &(dyn Fn(u64) + Sync)) -> Result<(Vec<u8>, String), String> {
    let t0 = Instant::now();
    let c = client();
    let result = match provider {
        "fal" => {
            let auth = format!("Key {key}");
            let sub = http_json(c.post(format!("https://queue.fal.run/{model}")).header("Authorization", &auth).json(body), "fal.ai").await?;
            let (status_url, resp_url, cancel_url) = (
                sub["status_url"].as_str().ok_or("fal.ai returned no status_url")?.to_string(),
                sub["response_url"].as_str().ok_or("fal.ai returned no response_url")?.to_string(),
                sub["cancel_url"].as_str().map(String::from),
            );
            loop {
                if let Err(e) = wait(cancel, tick, t0, 2).await {
                    if let Some(u) = &cancel_url {
                        let _ = c.put(u).header("Authorization", &auth).send().await;
                    }
                    return Err(e);
                }
                if t0.elapsed() > Duration::from_secs(30 * 60) {
                    return Err("fal.ai took longer than 30 minutes.".into());
                }
                let s = http_json(c.get(&status_url).header("Authorization", &auth), "fal.ai").await?;
                if s["status"] == "COMPLETED" {
                    break;
                }
            }
            http_json(c.get(&resp_url).header("Authorization", &auth), "fal.ai").await?
        }
        "replicate" => {
            let auth = format!("Bearer {key}");
            let mut p = http_json(c.post(format!("https://api.replicate.com/v1/models/{model}/predictions")).header("Authorization", &auth).header("Prefer", "wait=30").json(body), "Replicate").await?;
            let get = p["urls"]["get"].as_str().ok_or("Replicate returned no polling url")?.to_string();
            let cancel_url = p["urls"]["cancel"].as_str().map(String::from);
            loop {
                match p["status"].as_str().unwrap_or("") {
                    "succeeded" | "successful" => break,
                    "failed" | "canceled" => return Err(format!("Replicate: {}", p["error"].as_str().unwrap_or("generation failed"))),
                    _ => {}
                }
                if let Err(e) = wait(cancel, tick, t0, 2).await {
                    if let Some(u) = &cancel_url {
                        let _ = c.post(u).header("Authorization", &auth).send().await;
                    }
                    return Err(e);
                }
                if t0.elapsed() > Duration::from_secs(30 * 60) {
                    return Err("Replicate took longer than 30 minutes.".into());
                }
                p = http_json(c.get(&get).header("Authorization", &auth), "Replicate").await?;
            }
            p["output"].clone()
        }
        "openai" => {
            let mut req = body.clone();
            req["model"] = json!(model);
            let r = http_json(c.post("https://api.openai.com/v1/images/generations").timeout(Duration::from_secs(600)).header("Authorization", format!("Bearer {key}")).json(&req), "OpenAI").await?;
            return decode_image(&find_b64_image(&r).ok_or("OpenAI returned no image")?);
        }
        "gemini" => {
            let mut req = body.clone();
            req["model"] = json!(model);
            let r = http_json(c.post("https://generativelanguage.googleapis.com/v1beta/interactions").timeout(Duration::from_secs(600)).header("x-goog-api-key", key).json(&req), "Gemini").await?;
            return decode_image(&find_b64_image(&r).ok_or_else(|| format!("Gemini returned no image: {}", r.to_string().chars().take(300).collect::<String>()))?);
        }
        other => return Err(format!("Unknown provider: {other}")),
    };
    let url = find_media_url(&result).ok_or_else(|| format!("The provider finished but returned no video or image: {}", result.to_string().chars().take(300).collect::<String>()))?;
    download(&url).await
}

const RUNPOD_API: &str = "https://api.runpod.ai/v2";
/// Serverless A40 price per hour (USD), the default for the `runpod_gpu_dph` setting.
pub const RUNPOD_DEFAULT_DPH: f64 = 1.22;

/// RunPod worker-comfyui request body from a filled ComfyUI API graph. Random seeds are resolved here. Inline images
/// (our `AIVSLoadImageBase64` node, or a `LoadImage` whose image is base64 data instead of a file name) become a plain
/// `LoadImage` with a generated file name, and the data moves into `input.images` (the worker has no AIVS nodes).
pub fn runpod_body(graph: &Value) -> Value {
    let mut g = graph.clone();
    crate::local::randomize_seeds(&mut g);
    let mut images: Vec<Value> = vec![];
    if let Some(obj) = g.as_object_mut() {
        let mut ids: Vec<String> = obj.keys().cloned().collect();
        ids.sort_by_key(|k| k.parse::<u64>().unwrap_or(u64::MAX));
        for id in ids {
            let node = &mut obj[&id];
            let data = match node["class_type"].as_str() {
                Some("AIVSLoadImageBase64") => node["inputs"]["image_b64"].as_str().map(String::from),
                Some("LoadImage") => node["inputs"]["image"].as_str().filter(|s| s.len() > 256).map(String::from),
                _ => None,
            };
            if let Some(d) = data.filter(|d| !d.is_empty()) {
                let name = format!("aivs_input_{}.png", images.len() + 1);
                node["class_type"] = json!("LoadImage");
                node["inputs"] = json!({ "image": name });
                images.push(json!({ "name": name, "image": d }));
            }
        }
    }
    let mut input = json!({ "workflow": g });
    if !images.is_empty() {
        input["images"] = Value::Array(images);
    }
    json!({ "input": input, "policy": { "executionTimeout": 1_200_000 } })
}

/// File bytes and extension from a COMPLETED RunPod status (`output.images[]`, base64 data, ext from the file name).
/// A video wins over an image when the worker returns both.
pub fn runpod_extract(status: &Value) -> Result<(Vec<u8>, String), String> {
    use base64::Engine;
    let imgs = status["output"]["images"].as_array().filter(|a| !a.is_empty()).ok_or_else(|| format!("RunPod finished but returned no file: {}", status["output"].to_string().chars().take(300).collect::<String>()))?;
    let ext_of = |i: &Value| i["filename"].as_str().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    let pick = imgs.iter().find(|i| matches!(ext_of(i).as_str(), "mp4" | "webm" | "mov" | "gif")).unwrap_or(&imgs[0]);
    let data = pick["data"].as_str().ok_or("RunPod returned the file as a link, not as data. Use a worker-comfyui build that returns base64.")?;
    let data = data.split_once("base64,").map(|(_, d)| d).unwrap_or(data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| format!("bad RunPod file data: {e}"))?;
    let ext = Some(ext_of(pick)).filter(|e| !e.is_empty()).unwrap_or_else(|| if bytes.starts_with(&[0xFF, 0xD8]) { "jpg" } else { "png" }.into());
    Ok((bytes, ext))
}

/// Cost of one RunPod job in USD: billed execution time at the GPU's hourly rate. Cold start (`delayTime`, which also
/// includes queue waiting) is deliberately not counted, so this is a lower bound; the real bill is on the RunPod dashboard.
pub fn runpod_cost(status: &Value, dph: f64) -> f64 {
    status["executionTime"].as_f64().unwrap_or(0.0) / 1000.0 * dph / 3600.0
}

fn runpod_err(status: &Value) -> String {
    let e = &status["error"];
    let msg = e.as_str().map(String::from).unwrap_or_else(|| if e.is_null() { status["output"]["error"].as_str().unwrap_or("the job failed").to_string() } else { e.to_string() });
    format!("RunPod: {msg}")
}

/// Sends one RunPod request; returns the HTTP status and the JSON body (a transport failure is the only error).
async fn rp_send(r: reqwest::RequestBuilder) -> Result<(u16, Value), String> {
    let resp = r.send().await.map_err(|e| format!("RunPod: {e}"))?;
    let status = resp.status().as_u16();
    // a finished video comes back as several MB of base64 inside the JSON; no size cap here
    Ok((status, resp.json().await.unwrap_or(Value::Null)))
}

fn rp_check(status: u16, body: Value) -> Result<Value, String> {
    if (200..300).contains(&status) {
        return Ok(body);
    }
    let msg = body["error"].as_str().map(String::from).unwrap_or_else(|| body.to_string());
    Err(match status {
        401 | 403 => format!("RunPod: the API key was rejected. Check it in Settings. ({msg})"),
        404 => format!("RunPod: endpoint not found. Check the endpoint id in Settings. ({msg})"),
        _ => format!("RunPod: HTTP {status} {msg}"),
    })
}

async fn rp_json(r: reqwest::RequestBuilder) -> Result<Value, String> {
    let (status, body) = rp_send(r).await?;
    rp_check(status, body)
}

/// Runs a filled ComfyUI graph on a RunPod Serverless worker-comfyui endpoint. Returns (bytes, ext, cost in USD).
/// The endpoint is woken by the caller (`runpod::ensure_awake`); right after a wake-up the runtime answers 409 for a while,
/// so the submit is retried with backoff (`runpod::retry_delay`, 90 s in total). `note` receives a short status text.
pub async fn run_runpod(endpoint: &str, key: &str, graph: &Value, dph: f64, cancel: &AtomicBool, tick: &(dyn Fn(u64) + Sync), note: &(dyn Fn(&str) + Sync)) -> Result<(Vec<u8>, String, f64), String> {
    let t0 = Instant::now();
    let c = reqwest::Client::builder().user_agent("AI-Video-Studio").timeout(Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
    let auth = format!("Bearer {key}");
    let body = runpod_body(graph);
    let mut attempt = 0u32;
    let sub = loop {
        let (status, resp) = rp_send(c.post(format!("{RUNPOD_API}/{endpoint}/run")).header("Authorization", &auth).json(&body)).await?;
        if !crate::runpod::is_paused(status) {
            break rp_check(status, resp)?;
        }
        let Some(d) = crate::runpod::retry_delay(attempt, t0.elapsed()) else {
            return Err("RunPod is asleep and did not wake up within 90 seconds. Press \"Wake up RunPod\" in Settings and try again.".into());
        };
        attempt += 1;
        note("Waking up RunPod…");
        wait(cancel, tick, t0, d.as_secs()).await?;
    };
    let job = sub["id"].as_str().ok_or("RunPod returned no job id")?.to_string();
    let mut failures = 0;
    loop {
        if let Err(e) = wait(cancel, tick, t0, 2).await {
            let _ = c.post(format!("{RUNPOD_API}/{endpoint}/cancel/{job}")).header("Authorization", &auth).send().await;
            return Err(e);
        }
        if t0.elapsed() > Duration::from_secs(30 * 60) {
            let _ = c.post(format!("{RUNPOD_API}/{endpoint}/cancel/{job}")).header("Authorization", &auth).send().await;
            return Err("RunPod took longer than 30 minutes.".into());
        }
        let s = match rp_json(c.get(format!("{RUNPOD_API}/{endpoint}/status/{job}")).header("Authorization", &auth)).await {
            Ok(s) => { failures = 0; s }
            // a dropped connection while polling should not lose a paid job
            Err(e) if failures < 4 && !e.contains("rejected") && !e.contains("not found") => { failures += 1; continue; }
            Err(e) => return Err(e),
        };
        match s["status"].as_str().unwrap_or("") {
            "COMPLETED" => {
                let (bytes, ext) = runpod_extract(&s)?;
                return Ok((bytes, ext, runpod_cost(&s, dph)));
            }
            "FAILED" => return Err(runpod_err(&s)),
            "CANCELLED" => return Err("RunPod: the job was cancelled.".into()),
            "TIMED_OUT" => return Err("RunPod: the job timed out on the worker (20 minute limit).".into()),
            _ => {}
        }
    }
}

/// Cheap key check used by the wizard and Settings. `extra` is the RunPod endpoint id (ignored by the others).
pub async fn test_key(provider: &str, key: &str, extra: &str) -> Result<String, String> {
    let c = client();
    match provider {
        "runpod" => {
            let id = extra.trim();
            if id.is_empty() {
                return Err("RunPod: enter the endpoint id first.".into());
            }
            rp_json(c.get(format!("{RUNPOD_API}/{id}/health")).header("Authorization", format!("Bearer {key}"))).await?;
            Ok("RunPod key and endpoint accepted".into())
        }
        "replicate" => {
            http_json(c.get("https://api.replicate.com/v1/account").header("Authorization", format!("Bearer {key}")), "Replicate").await?;
            Ok("Replicate key accepted".into())
        }
        "fal" => {
            // an invalid body to a real model: 401/403 means a bad key, anything else means the key was accepted
            let r = c.post("https://queue.fal.run/fal-ai/flux/schnell").header("Authorization", format!("Key {key}")).json(&json!({})).send().await.map_err(|e| e.to_string())?;
            match r.status().as_u16() {
                401 | 403 => Err("fal.ai: the API key was rejected.".into()),
                _ => Ok("fal.ai key accepted".into()),
            }
        }
        "openai" => {
            http_json(c.get("https://api.openai.com/v1/models").header("Authorization", format!("Bearer {key}")), "OpenAI").await?;
            Ok("OpenAI key accepted".into())
        }
        "gemini" => {
            http_json(c.get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=1").header("x-goog-api-key", key), "Gemini").await?;
            Ok("Gemini key accepted".into())
        }
        other => Err(format!("Unknown provider: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_keeps_types_and_drops_missing() {
        let tpl = json!({"prompt": "{prompt}", "image_size": {"width": "{width}", "height": "{height}"}, "seed": "{seed}", "steps": 4, "note": "a {prompt}!"});
        let mut p = Map::new();
        p.insert("prompt".into(), json!("fox"));
        p.insert("width".into(), json!(1024));
        p.insert("height".into(), json!(768));
        let r = render(&tpl, &p);
        assert_eq!(r["image_size"]["width"], json!(1024));
        assert!(r.get("seed").is_none());
        assert_eq!(r["note"], json!("a fox!"));
        assert_eq!(r["steps"], json!(4));
    }

    #[test]
    fn media_url_is_found_and_video_wins() {
        let fal = json!({"images": [{"url": "https://x/a.png"}], "video": {"url": "https://x/v.mp4?sig=1"}});
        assert_eq!(find_media_url(&fal).as_deref(), Some("https://x/v.mp4?sig=1"));
        assert_eq!(find_media_url(&json!(["https://r/out-0.webp"])).as_deref(), Some("https://r/out-0.webp"));
        assert_eq!(find_media_url(&json!({"text": "https://x/page.html"})), None);
    }

    #[test]
    fn b64_image_is_found() {
        assert_eq!(find_b64_image(&json!({"data": [{"b64_json": "AAA"}]})).as_deref(), Some("AAA"));
        assert_eq!(find_b64_image(&json!({"steps": [{"content": [{"type": "image", "mime_type": "image/png", "data": "BBB"}]}]})).as_deref(), Some("BBB"));
        assert_eq!(find_b64_image(&json!({"text": "hi", "data": "x"})), None);
    }

    #[test]
    fn runpod_body_moves_inline_images_and_resolves_seeds() {
        let long = "A".repeat(400);
        let g = json!({
            "1": {"class_type": "KSampler", "inputs": {"seed": "__RANDOM_INT__", "steps": 4}},
            "2": {"class_type": "AIVSLoadImageBase64", "inputs": {"image_b64": "QUJD"}},
            "3": {"class_type": "LoadImage", "inputs": {"image": long}},
            "4": {"class_type": "LoadImage", "inputs": {"image": "already_on_worker.png"}}
        });
        let b = runpod_body(&g);
        assert!(b["input"]["workflow"]["1"]["inputs"]["seed"].is_i64());
        assert_eq!(b["input"]["workflow"]["2"]["class_type"], "LoadImage");
        assert_eq!(b["input"]["workflow"]["2"]["inputs"]["image"], "aivs_input_1.png");
        assert_eq!(b["input"]["workflow"]["3"]["inputs"]["image"], "aivs_input_2.png");
        assert_eq!(b["input"]["workflow"]["4"]["inputs"]["image"], "already_on_worker.png");
        let imgs = b["input"]["images"].as_array().unwrap();
        assert_eq!(imgs.len(), 2);
        assert_eq!(imgs[0], json!({"name": "aivs_input_1.png", "image": "QUJD"}));
        assert_eq!(b["policy"]["executionTimeout"], 1_200_000);
        assert!(runpod_body(&json!({"1": {"class_type": "KSampler", "inputs": {}}}))["input"].get("images").is_none());
    }

    #[test]
    fn runpod_result_is_extracted_and_costed() {
        let st = json!({"status": "COMPLETED", "delayTime": 40000, "executionTime": 185000,
            "output": {"images": [{"filename": "aivs_00001_.mp4", "type": "base64", "data": "QUJD"}]}});
        let (bytes, ext) = runpod_extract(&st).unwrap();
        assert_eq!((bytes.as_slice(), ext.as_str()), (&b"ABC"[..], "mp4"));
        let png = json!({"output": {"images": [{"filename": "a.PNG", "type": "base64", "data": "QUJD"}]}});
        assert_eq!(runpod_extract(&png).unwrap().1, "png");
        assert!(runpod_extract(&json!({"output": {}})).is_err());
        assert!((runpod_cost(&st, 1.22) - 185.0 * 1.22 / 3600.0).abs() < 1e-9);
        assert_eq!(runpod_err(&json!({"error": "missing model x.safetensors"})), "RunPod: missing model x.safetensors");
    }

    #[test]
    fn secret_names_exist() {
        assert_eq!(secret_name("runpod"), Some("runpod_api_key"));
        assert_eq!(secret_name("fal"), Some("fal_key"));
        assert_eq!(secret_name("nope"), None);
    }

    /// Real RunPod run: wake, one FLUX image through run_runpod, sleep. Costs about 0.02 USD. Run with --ignored --nocapture.
    #[tokio::test]
    #[ignore = "spends real RunPod money"]
    async fn real_runpod_flux_roundtrip() {
        let key = std::fs::read_to_string(dirs_home().join(".aivs-keys").join("runpod_api_key.txt")).unwrap().trim().to_string();
        let id = "n1q8r5vkmex43u";
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workflows");
        let (all, _) = crate::workflows::load_all(&dir, &dir.join("none"));
        let wf = all.iter().find(|w| w.meta.id == "runpod-flux3-klein-t2i").unwrap();
        let mut p = Map::new();
        p.insert("prompt".into(), json!("a red fox in the snow"));
        let graph = crate::workflows::fill(wf, &p);
        println!("{}", crate::runpod::wake(&key, id).await.unwrap());
        let cancel = AtomicBool::new(false);
        let r = run_runpod(id, &key, &graph, 1.22, &cancel, &|s| println!("tick {s}"), &|m| println!("note {m}")).await;
        println!("{}", crate::runpod::sleep(&key, id).await.unwrap());
        let (bytes, ext, cost) = r.unwrap();
        std::fs::write(std::env::temp_dir().join(format!("aivs_real.{ext}")), &bytes).unwrap();
        println!("OK bytes={} ext={ext} cost={cost}", bytes.len());
    }
    fn dirs_home() -> std::path::PathBuf { std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap()) }
}
