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
    let parts = split_text(text, MAX_PART);
    let mut wavs = vec![];
    for p in &parts { wavs.push(speak_one(p, voice).await?); }
    join_wavs(&wavs)
}

async fn speak_one(text: &str, voice: &Cfg) -> Result<Vec<u8>, String> {
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

/// Longest text part sent to the voice engine at once (characters). One long script made Voice Studio on a laptop GPU give up
/// after its 300 s compute limit, so long scripts are spoken in sentence-sized parts (same seed and speed) and joined.
const MAX_PART: usize = 220;

/// Splits a script at sentence ends into parts of at most `max` characters (a longer sentence is split at commas, then spaces).
pub fn split_text(text: &str, max: usize) -> Vec<String> {
    let mut sentences: Vec<String> = vec![];
    let mut cur = String::new();
    for ch in text.trim().chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\u{2026}' | '\u{37e}' | '\n') { sentences.push(std::mem::take(&mut cur)); }
    }
    if !cur.trim().is_empty() { sentences.push(cur); }
    let mut pieces: Vec<String> = vec![];
    for s in sentences {
        let s = s.trim().to_string();
        if s.is_empty() { continue; }
        if s.chars().count() <= max { pieces.push(s); continue; }
        let mut line = String::new();
        for w in s.split_inclusive(|c: char| c == ',' || c == ' ') {
            if line.chars().count() + w.chars().count() > max && !line.trim().is_empty() { pieces.push(line.trim().to_string()); line.clear(); }
            line.push_str(w);
        }
        if !line.trim().is_empty() { pieces.push(line.trim().to_string()); }
    }
    let mut parts: Vec<String> = vec![];
    for p in pieces {
        match parts.last_mut() {
            Some(last) if last.chars().count() + 1 + p.chars().count() <= max => { last.push(' '); last.push_str(&p); }
            _ => parts.push(p),
        }
    }
    if parts.is_empty() { parts.push(text.trim().to_string()); }
    parts
}

/// (sample rate, channels, bits per sample, sound data) of a PCM WAV.
fn wav_parts(b: &[u8]) -> Option<(u32, u16, u16, &[u8])> {
    if b.len() < 44 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" { return None; }
    let (mut i, mut fmt) = (12usize, None);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        if id == b"fmt " && i + 8 + 16 <= b.len() {
            fmt = Some((u32::from_le_bytes([b[i + 12], b[i + 13], b[i + 14], b[i + 15]]), u16::from_le_bytes([b[i + 10], b[i + 11]]), u16::from_le_bytes([b[i + 22], b[i + 23]])));
        }
        if id == b"data" {
            let end = (i + 8 + len).min(b.len());
            return fmt.map(|(r, c, bits)| (r, c, bits, &b[i + 8..end]));
        }
        i += 8 + len + (len & 1);
    }
    None
}

/// Joins WAV parts of the same format into one WAV with a quarter second of silence between them.
pub fn join_wavs(parts: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if parts.len() == 1 { return Ok(parts[0].clone()); }
    let mut data: Vec<u8> = vec![];
    let mut fmt: Option<(u32, u16, u16)> = None;
    for p in parts {
        let (r, c, bits, d) = wav_parts(p).ok_or("The voice returned a sound file that cannot be joined.")?;
        match fmt {
            None => fmt = Some((r, c, bits)),
            Some(f) if f != (r, c, bits) => return Err("The voice parts have different sound formats.".into()),
            _ => {}
        }
        if !data.is_empty() { data.extend(std::iter::repeat(0u8).take((r as usize / 4) * c as usize * (bits as usize / 8))); }
        data.extend_from_slice(d);
    }
    let (r, c, bits) = fmt.ok_or("No voice parts to join.")?;
    let align = c as u32 * bits as u32 / 8;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&c.to_le_bytes());
    out.extend_from_slice(&r.to_le_bytes()); out.extend_from_slice(&(r * align).to_le_bytes()); out.extend_from_slice(&(align as u16).to_le_bytes()); out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data"); out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    Ok(out)
}

/// Is Voice Studio on this PC (running or installed)?
pub async fn local_available() -> bool { alive(&client()).await || exe().is_some() }

/// One text part on the RunPod voice endpoint: submit (retrying while the endpoint wakes up), wait, return the WAV bytes.
async fn runpod_one(c: &reqwest::Client, key: &str, endpoint: &str, input: &serde_json::Value, say: &(dyn Fn(&str) + Sync)) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
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
}

/// Speaks `text` on the RunPod Voice Studio endpoint (same backend and settings as the PC). The endpoint is parked
/// (workers 0) between uses: it is woken here and parked again when the audio is back, so only the seconds used are billed.
pub async fn speak_runpod(key: &str, endpoint: &str, text: &str, voice: &Cfg, say: &(dyn Fn(&str) + Sync)) -> Result<Vec<u8>, String> {
    use base64::Engine;
    if !crate::runpod::valid_id(endpoint) { return Err("The RunPod voice endpoint id looks wrong. Check it in Settings.".into()); }
    let b64 = base64::engine::general_purpose::STANDARD;
    let sample = tokio::fs::read(&voice.file).await.map_err(|e| e.to_string())?;
    let ext = voice.file.extension().and_then(|x| x.to_str()).unwrap_or("wav").to_string();
    let sample_b64 = b64.encode(&sample);
    let input_for = |t: &str| {
        let mut input = serde_json::json!({ "text": t, "language": voice.language, "speed": voice.speed, "instruct": voice.instruct, "ref_text": voice.ref_text, "ref_audio_b64": sample_b64, "ref_ext": ext });
        if let Some(s) = voice.seed { input["seed"] = serde_json::json!(s); }
        input
    };
    let c = reqwest::Client::builder().user_agent("AI-Video-Studio").timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    say("Waking up the RunPod voice...");
    crate::runpod::set_max(key, endpoint, 1).await?;
    let res: Result<Vec<u8>, String> = async {
        let parts = split_text(text, MAX_PART);
        let mut wavs = vec![];
        for (n, part) in parts.iter().enumerate() {
            if parts.len() > 1 { say(&format!("RunPod voice: part {} of {}", n + 1, parts.len())); }
            wavs.push(runpod_one(&c, key, endpoint, &input_for(part), say).await?);
        }
        join_wavs(&wavs)
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

    #[test]
    fn long_scripts_are_split_at_sentences_and_joined() {
        let t = "Hello there. This is the second sentence! And a third one? Yes.";
        let p = split_text(t, 30);
        assert!(p.len() >= 2 && p.iter().all(|x| x.chars().count() <= 30), "{p:?}");
        assert_eq!(p.join(" "), t);
        assert_eq!(split_text("short", 220), vec!["short".to_string()]);
        let long = "word, ".repeat(100);
        assert!(split_text(&long, 50).iter().all(|x| x.chars().count() <= 50));
        let mk = |n: usize| { let mut w = b"RIFF\0\0\0\0WAVEfmt ".to_vec(); w.extend_from_slice(&16u32.to_le_bytes()); w.extend_from_slice(&1u16.to_le_bytes()); w.extend_from_slice(&1u16.to_le_bytes()); w.extend_from_slice(&24000u32.to_le_bytes()); w.extend_from_slice(&48000u32.to_le_bytes()); w.extend_from_slice(&2u16.to_le_bytes()); w.extend_from_slice(&16u16.to_le_bytes()); w.extend_from_slice(b"data"); w.extend_from_slice(&(n as u32).to_le_bytes()); w.extend(vec![1u8; n]); w };
        let j = join_wavs(&[mk(48000), mk(48000)]).unwrap();
        // 1 s + 0.25 s of silence + 1 s
        assert!((wav_seconds(&j) - 2.25).abs() < 1e-6);
        assert_eq!(join_wavs(&[mk(100)]).unwrap().len(), mk(100).len());
    }
}
