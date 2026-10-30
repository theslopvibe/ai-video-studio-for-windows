//! Hosted model APIs (fal.ai, Replicate): no GPU rental and no ComfyUI, you pay per result.
//! A hosted mode is a workflow JSON whose meta has `provider` and a `request` template; strings like "{width}" in the
//! template are replaced by the job's parameters. Model ids change often, so users can edit these files.
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PROVIDERS: &[(&str, &str)] = &[("fal", "fal_key"), ("replicate", "replicate_token"), ("openai", "openai_api_key"), ("gemini", "gemini_api_key")];

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

/// Cheap key check used by the wizard and Settings.
pub async fn test_key(provider: &str, key: &str) -> Result<String, String> {
    let c = client();
    match provider {
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
    fn secret_names_exist() {
        assert_eq!(secret_name("fal"), Some("fal_key"));
        assert_eq!(secret_name("nope"), None);
    }
}
