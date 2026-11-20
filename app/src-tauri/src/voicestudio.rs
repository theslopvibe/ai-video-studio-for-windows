//! Voice Studio (the user's own local app, engines OmniVoice / VoxCPM2): text to speech in a saved voice, on this PC.
//! Its API listens on 127.0.0.1:3900 without a key (loopback only). The app starts it when it is not running.
use crate::voices::Cfg;
use std::time::Duration;

const URL: &str = "http://127.0.0.1:3900";

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().connect_timeout(Duration::from_secs(5)).build().unwrap_or_default()
}

async fn alive(c: &reqwest::Client) -> bool {
    c.get(format!("{URL}/profiles")).timeout(Duration::from_secs(5)).send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

fn exe() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").ok()?).join("Programs").join("VoiceStudio").join("VoiceStudio.exe");
    p.exists().then_some(p)
}

/// Starts Voice Studio when it is not running and waits (up to 3 minutes) until its API answers.
pub async fn ensure_running(say: &(dyn Fn(&str) + Sync)) -> Result<(), String> {
    let c = client();
    if alive(&c).await { return Ok(()); }
    let exe = exe().ok_or("Voice Studio is not installed on this PC (looked for %LOCALAPPDATA%\\Programs\\VoiceStudio). Install it, or use another voice mode.")?;
    say("Starting Voice Studio…");
    std::process::Command::new(&exe).spawn().map_err(|e| format!("Could not start Voice Studio: {e}"))?;
    for i in 0..60 {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if alive(&c).await { return Ok(()); }
        if i % 5 == 4 { say(&format!("Waiting for Voice Studio… {}s", (i + 1) * 3)); }
    }
    Err("Voice Studio did not answer within 3 minutes.".into())
}

fn part_text(b: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    b.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
}

/// Speaks `text` in the saved voice and returns the WAV bytes. With a Voice Studio profile the engine uses the stored
/// reference, seed and consent; without one the saved sample is uploaded as the reference. The first call after a start
/// loads the model (several minutes on a laptop GPU), later ones take seconds.
pub async fn speak(text: &str, voice: &Cfg) -> Result<Vec<u8>, String> {
    let c = client();
    let boundary = format!("aivs{}", uuid::Uuid::new_v4().simple());
    let mut body: Vec<u8> = vec![];
    part_text(&mut body, &boundary, "text", text);
    part_text(&mut body, &boundary, "language", &voice.language);
    part_text(&mut body, &boundary, "speed", &format!("{}", voice.speed));
    part_text(&mut body, &boundary, "engine", "omnivoice");
    if !voice.instruct.is_empty() { part_text(&mut body, &boundary, "instruct", &voice.instruct); }
    if let Some(s) = voice.seed { part_text(&mut body, &boundary, "seed", &s.to_string()); }
    let profile_ok = !voice.profile_id.is_empty()
        && c.get(format!("{URL}/profiles/{}", voice.profile_id)).timeout(Duration::from_secs(10)).send().await.map(|r| r.status().is_success()).unwrap_or(false);
    if profile_ok {
        part_text(&mut body, &boundary, "profile_id", &voice.profile_id);
    } else {
        if !voice.ref_text.is_empty() { part_text(&mut body, &boundary, "ref_text", &voice.ref_text); }
        let bytes = tokio::fs::read(&voice.file).await.map_err(|e| e.to_string())?;
        let fname = voice.file.file_name().and_then(|n| n.to_str()).unwrap_or("voice.mp3");
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"ref_audio\"; filename=\"{fname}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(&bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let r = c.post(format!("{URL}/generate")).header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .timeout(Duration::from_secs(25 * 60)).body(body).send().await.map_err(|e| format!("Voice Studio: {e}"))?;
    let ok = r.status().is_success();
    let bytes = r.bytes().await.map_err(|e| e.to_string())?;
    if !ok { return Err(format!("Voice Studio refused the request: {}", String::from_utf8_lossy(&bytes).chars().take(300).collect::<String>())); }
    if bytes.len() < 1000 { return Err("Voice Studio returned no sound.".into()); }
    Ok(bytes.to_vec())
}

/// Is Voice Studio on this PC (running or installed)?
pub async fn local_available() -> bool { alive(&client()).await || exe().is_some() }

/// Speaks `text` on the RunPod Voice Studio endpoint (same backend and settings as the PC). The endpoint is parked
/// (workers 0) between uses: it is woken here and parked again when the audio is back, so only the seconds used are billed.
pub async fn speak_runpod(key: &str, endpoint: &str, text: &str, voice: &Cfg, say: &(dyn Fn(&str) + Sync)) -> Result<Vec<u8>, String> {
    use base64::Engine;
    if !crate::runpod::valid_id(endpoint) { return Err("The RunPod voice endpoint id looks wrong. Check it in Settings.".into()); }
    let b64 = base64::engine::general_purpose::STANDARD;
    let sample = tokio::fs::read(&voice.file).await.map_err(|e| e.to_string())?;
    let ext = voice.file.extension().and_then(|x| x.to_str()).unwrap_or("wav").to_string();
    let mut input = serde_json::json!({ "text": text, "language": voice.language, "speed": voice.speed, "instruct": voice.instruct, "ref_text": voice.ref_text, "ref_audio_b64": b64.encode(&sample), "ref_ext": ext });
    if let Some(s) = voice.seed { input["seed"] = serde_json::json!(s); }
    let c = reqwest::Client::builder().user_agent("AI-Video-Studio").timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    say("Waking up the RunPod voice...");
    crate::runpod::set_max(key, endpoint, 1).await?;
    let res: Result<Vec<u8>, String> = async {
        // right after waking, RunPod answers "paused" for a few seconds: retry the submit for about a minute
        let mut sub = serde_json::Value::Null;
        for attempt in 0..14 {
            sub = c.post(format!("https://api.runpod.ai/v2/{endpoint}/run")).bearer_auth(key).json(&serde_json::json!({ "input": input }))
                .send().await.map_err(|e| format!("RunPod: {e}"))?.json().await.map_err(|e| format!("RunPod: {e}"))?;
            if sub["code"].as_str() != Some("ENDPOINT_PAUSED") || attempt == 13 { break; }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let id = sub["id"].as_str().ok_or_else(|| format!("RunPod did not accept the job: {sub}"))?.to_string();
        let t0 = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if t0.elapsed() > Duration::from_secs(45 * 60) { return Err("The RunPod voice did not answer within 45 minutes.".into()); }
            let s: serde_json::Value = match c.get(format!("https://api.runpod.ai/v2/{endpoint}/status/{id}")).bearer_auth(key).send().await { Ok(r) => r.json().await.unwrap_or_default(), Err(_) => continue };
            match s["status"].as_str().unwrap_or("") {
                "COMPLETED" => {
                    if let Some(e) = s["output"]["error"].as_str() { return Err(format!("RunPod voice: {e}")); }
                    let a = s["output"]["audio_b64"].as_str().ok_or("RunPod voice: no audio in the answer.")?;
                    return b64.decode(a).map_err(|_| "RunPod voice: the audio was damaged.".to_string());
                }
                "FAILED" | "CANCELLED" | "TIMED_OUT" => return Err(format!("RunPod voice job {}: {}", s["status"], s["error"])),
                other => say(&format!("RunPod voice: {} ({}s)", other.to_lowercase().replace('_', " "), t0.elapsed().as_secs())),
            }
        }
    }.await;
    // park it again whatever happened: an idle worker is billed
    if let Err(e) = crate::runpod::set_max(key, endpoint, 0).await { crate::log::info(&format!("runpod: could not park the voice endpoint: {e}")); }
    res
}

/// Length of a PCM WAV in seconds (0 when it is not a WAV we can read).
pub fn wav_seconds(bytes: &[u8]) -> f64 {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" { return 0.0; }
    let (mut i, mut rate, mut align) = (12usize, 0u32, 0u32);
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
        if id == b"fmt " && i + 8 + 16 <= bytes.len() {
            rate = u32::from_le_bytes([bytes[i + 12], bytes[i + 13], bytes[i + 14], bytes[i + 15]]);
            align = u16::from_le_bytes([bytes[i + 20], bytes[i + 21]]) as u32;
        }
        if id == b"data" { return if rate > 0 && align > 0 { len.min(bytes.len() - i - 8) as f64 / (rate as f64 * align as f64) } else { 0.0 }; }
        i += 8 + len + (len & 1);
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_length_is_read_from_the_header() {
        let mut w = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        w.extend_from_slice(&16u32.to_le_bytes()); w.extend_from_slice(&1u16.to_le_bytes()); w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&24000u32.to_le_bytes()); w.extend_from_slice(&48000u32.to_le_bytes()); w.extend_from_slice(&2u16.to_le_bytes()); w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data"); w.extend_from_slice(&96000u32.to_le_bytes()); w.extend_from_slice(&vec![0u8; 96000]);
        assert!((wav_seconds(&w) - 2.0).abs() < 1e-6);
        assert_eq!(wav_seconds(b"nope"), 0.0);
    }
}
