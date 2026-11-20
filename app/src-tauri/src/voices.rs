//! Saved voices: a short sample of a voice with a name, kept on this PC, so "use Vasilis's voice" works in any later job.
//! A job asks for one with {"audio": "voice:<name>"}; the sample is then sent to the clone workflow like any uploaded sound.
use crate::AppState;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const AUDIO_EXT: &[&str] = &["mp3", "wav", "flac", "ogg", "opus", "m4a"];

fn dir(st: &AppState) -> PathBuf { st.data_dir.join("voices") }

/// `Vasilis` -> `vasilis`; only letters, digits, `-` and `_` survive (the name is also a file name).
fn slug(name: &str) -> Result<String, String> {
    let s: String = name.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() && c.is_ascii() || c == '-' || c == '_' { c } else if c == ' ' { '-' } else { '\0' }).filter(|c| *c != '\0').collect();
    if s.is_empty() || s.len() > 40 { return Err("Give the voice a short name (letters and digits, up to 40 characters), for example \"vasilis\".".into()); }
    Ok(s)
}

/// Voice Studio style tags such as "american accent, male": letters, spaces and commas only, short.
fn clean_instruct(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphabetic() || *c == ' ' || *c == ',').take(120).collect::<String>().trim().to_string()
}

/// The exact words spoken in the sample (any language, UTF-8). Sent with the sample so no speech-to-text model is needed
/// and the voice comes out the same everywhere.
fn clean_ref_text(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(1000).collect::<String>().trim().to_string()
}

fn index(st: &AppState) -> serde_json::Map<String, Value> {
    st.db.kv_get("voices").and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

pub fn list(st: &AppState) -> Vec<Value> {
    index(st).iter().filter(|(_, v)| v["file"].as_str().is_some_and(|f| Path::new(f).exists()))
        .map(|(k, v)| json!({ "name": k, "saved": v["saved"], "speed": v["speed"], "seed": v["seed"], "voice_studio_profile": v["profile_id"], "language": v["language"], "instruct": v["instruct"], "ref_text": v["ref_text"] })).collect()
}

pub fn save(st: &AppState, name: &str, audio_path: &str, consent: bool, extra: &Value) -> Result<Value, String> {
    if !consent {
        return Err("Ask the user first: is this their own voice, or does the person have permission for it to be cloned? Then call again with consent_confirmed=true. Never clone a voice to deceive people.".into());
    }
    let name = slug(name)?;
    let ext = Path::new(audio_path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if !AUDIO_EXT.contains(&ext.as_str()) { return Err("The sample must be a sound file (mp3, wav, flac, ogg, m4a).".into()); }
    // stash_input does the path checks (local drive only, no links, size limit); the copy is then moved to the voices folder
    let tmp = crate::stash_input(st, Path::new(audio_path), &format!("voice-{name}"))?;
    let tmp = PathBuf::from(tmp.trim_start_matches("file:"));
    let d = dir(st);
    std::fs::create_dir_all(&d).map_err(|x| x.to_string())?;
    let dest = d.join(format!("{name}.{ext}"));
    std::fs::copy(&tmp, &dest).map_err(|x| x.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let mut idx = index(st);
    if let Some(old) = idx.get(&name).and_then(|v| v["file"].as_str()).map(PathBuf::from) { if old != dest { let _ = std::fs::remove_file(old); } }
    // settings that make the voice come out the same every time: Voice Studio profile, speed, seed, language
    let num = |k: &str| extra[k].as_f64().filter(|x| x.is_finite());
    idx.insert(name.clone(), json!({
        "file": dest.to_string_lossy(), "saved": chrono_now(),
        "profile_id": extra["voice_studio_profile"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(64).collect::<String>(),
        "speed": num("speed").map(|s| s.clamp(0.5, 1.5)), "seed": num("seed").map(|s| s as i64),
        "language": extra["language"].as_str().unwrap_or("English").chars().filter(|c| c.is_ascii_alphabetic()).take(30).collect::<String>(),
        "instruct": clean_instruct(extra["instruct"].as_str().unwrap_or("")),
        "ref_text": clean_ref_text(extra["ref_text"].as_str().unwrap_or("")),
    }));
    st.db.kv_set("voices", &Value::Object(idx).to_string()).map_err(|x| x.to_string())?;
    Ok(json!({ "message": format!("Voice \"{name}\" saved. Use it with generate_video: workflow chatterbox-clone, inputs {{\"audio\":\"voice:{name}\"}}."), "name": name }))
}

/// The saved voices with their sample bytes, for a profile pack: (name, extension, settings, sample).
pub fn export_all(st: &AppState, only: &[String]) -> Vec<(String, String, Value, Vec<u8>)> {
    index(st).into_iter().filter(|(k, _)| only.is_empty() || only.contains(k)).filter_map(|(k, v)| {
        let f = PathBuf::from(v["file"].as_str()?);
        let bytes = std::fs::read(&f).ok()?;
        let ext = f.extension().and_then(|x| x.to_str()).unwrap_or("mp3").to_string();
        Some((k, ext, v, bytes))
    }).collect()
}

/// Stores a voice that arrived inside a profile pack (the sample as bytes). Same limits as `save`.
pub fn put(st: &AppState, name: &str, ext: &str, bytes: &[u8], extra: &Value) -> Result<String, String> {
    let name = slug(name)?;
    let ext = ext.to_ascii_lowercase();
    if !AUDIO_EXT.contains(&ext.as_str()) { return Err("The sample must be a sound file (mp3, wav, flac, ogg, m4a).".into()); }
    if bytes.is_empty() || bytes.len() > 30 * 1024 * 1024 { return Err("The voice sample is empty or larger than 30 MB.".into()); }
    let d = dir(st);
    std::fs::create_dir_all(&d).map_err(|x| x.to_string())?;
    let dest = d.join(format!("{name}.{ext}"));
    std::fs::write(&dest, bytes).map_err(|x| x.to_string())?;
    let mut idx = index(st);
    if let Some(old) = idx.get(&name).and_then(|v| v["file"].as_str()).map(PathBuf::from) { if old != dest { let _ = std::fs::remove_file(old); } }
    let num = |k: &str| extra[k].as_f64().filter(|x| x.is_finite());
    idx.insert(name.clone(), json!({
        "file": dest.to_string_lossy(), "saved": chrono_now(),
        "profile_id": extra["profile_id"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(64).collect::<String>(),
        "speed": num("speed").map(|s| s.clamp(0.5, 1.5)), "seed": num("seed").map(|s| s as i64),
        "language": extra["language"].as_str().unwrap_or("English").chars().filter(|c| c.is_ascii_alphabetic()).take(30).collect::<String>(),
        "instruct": clean_instruct(extra["instruct"].as_str().unwrap_or("")),
        "ref_text": clean_ref_text(extra["ref_text"].as_str().unwrap_or("")),
    }));
    st.db.kv_set("voices", &Value::Object(idx).to_string()).map_err(|x| x.to_string())?;
    Ok(name)
}

pub fn delete(st: &AppState, name: &str) -> Result<Value, String> {
    let name = slug(name)?;
    let mut idx = index(st);
    let Some(v) = idx.remove(&name) else { return Err(format!("No saved voice called \"{name}\".")) };
    if let Some(f) = v["file"].as_str() { let _ = std::fs::remove_file(f); }
    st.db.kv_set("voices", &Value::Object(idx).to_string()).map_err(|x| x.to_string())?;
    Ok(json!({ "message": format!("Voice \"{name}\" deleted.") }))
}

/// `voice:<name>` -> a `file:` reference to a fresh copy of the sample in the inputs folder.
pub fn resolve(st: &AppState, name: &str) -> Result<String, String> {
    let name = slug(name)?;
    let idx = index(st);
    let file = idx.get(&name).and_then(|v| v["file"].as_str()).filter(|f| Path::new(f).exists()).ok_or_else(|| {
        let names: Vec<String> = idx.keys().cloned().collect();
        format!("No saved voice called \"{name}\". Saved voices: {}.", if names.is_empty() { "none".into() } else { names.join(", ") })
    })?;
    crate::stash_input(st, Path::new(file), &format!("voice-{name}-{}", uuid::Uuid::new_v4().simple()))
}

fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{s}")
}

/// The saved settings of a voice, for the Voice Studio engine.
pub struct Cfg { pub file: PathBuf, pub profile_id: String, pub speed: f64, pub seed: Option<i64>, pub language: String, pub instruct: String, pub ref_text: String }

pub fn get(st: &AppState, name: &str) -> Result<Cfg, String> {
    let name = slug(name)?;
    let idx = index(st);
    let v = idx.get(&name).ok_or_else(|| {
        let names: Vec<String> = idx.keys().cloned().collect();
        format!("No saved voice called \"{name}\". Saved voices: {}.", if names.is_empty() { "none".into() } else { names.join(", ") })
    })?;
    let file = PathBuf::from(v["file"].as_str().unwrap_or(""));
    if !file.exists() { return Err(format!("The sample of voice \"{name}\" is missing; save the voice again.")); }
    Ok(Cfg { file, profile_id: v["profile_id"].as_str().unwrap_or("").into(), speed: v["speed"].as_f64().unwrap_or(1.0), seed: v["seed"].as_i64(), language: v["language"].as_str().filter(|s| !s.is_empty()).unwrap_or("English").into(), instruct: v["instruct"].as_str().unwrap_or("").into(), ref_text: v["ref_text"].as_str().unwrap_or("").into() })
}
