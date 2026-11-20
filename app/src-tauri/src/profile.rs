//! Profile packs: one .json file that carries what a team shares so everyone gets the same set-up:
//! saved voices (sample + the settings that make them sound identical), the RunPod instances (name, endpoint id)
//! and which one is active. The RunPod API keys can travel inside too, but only encrypted with a password
//! (AES-256-GCM, key from PBKDF2). Loading a pack is only possible from the app's own window, after a preview.
use crate::AppState;
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};

const FORMAT: &str = "aivs-profile";
const ITER: u32 = 600_000;
/// (slot, secret name, endpoint setting)
const SLOTS: [(&str, &str, &str); 2] = [("1", "runpod_api_key", "runpod_endpoint_id"), ("2", "runpod_api_key_2", "runpod_endpoint_id_2")];
fn tts_setting(slot: &str) -> &'static str { if slot == "2" { "runpod_tts_endpoint_id_2" } else { "runpod_tts_endpoint_id" } }

fn derive(password: &str, salt: &[u8], iter: u32) -> [u8; 32] {
    let mut k = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iter, &mut k);
    k
}

fn seal(password: &str, plain: &[u8]) -> Result<Value, String> {
    let salt = *uuid::Uuid::new_v4().as_bytes();
    let nonce_src = *uuid::Uuid::new_v4().as_bytes();
    let nonce = &nonce_src[..12];
    let c = Aes256Gcm::new_from_slice(&derive(password, &salt, ITER)).map_err(|x| x.to_string())?;
    let ct = c.encrypt(Nonce::from_slice(nonce), plain).map_err(|_| "Encryption failed.".to_string())?;
    Ok(json!({ "kdf": "pbkdf2-sha256", "iter": ITER, "salt": B64.encode(salt), "nonce": B64.encode(nonce), "ct": B64.encode(ct) }))
}

fn open_sealed(password: &str, s: &Value) -> Result<Vec<u8>, String> {
    let get = |k: &str| B64.decode(s[k].as_str().unwrap_or("")).map_err(|_| "The profile file is damaged.".to_string());
    let iter = s["iter"].as_u64().unwrap_or(0) as u32;
    if !(100_000..=5_000_000).contains(&iter) { return Err("The profile file is damaged.".into()); }
    let (salt, nonce, ct) = (get("salt")?, get("nonce")?, get("ct")?);
    if nonce.len() != 12 { return Err("The profile file is damaged.".into()); }
    let c = Aes256Gcm::new_from_slice(&derive(password, &salt, iter)).map_err(|x| x.to_string())?;
    c.decrypt(Nonce::from_slice(&nonce), ct.as_ref()).map_err(|_| "Wrong password.".to_string())
}

fn clean_id(s: &str) -> String { s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(64).collect() }
/// Instructions for whoever loads the pack: plain text, shown in the preview and never run or sent to a model.
fn clean_notes(s: &str) -> String { s.chars().filter(|c| !c.is_control() || *c == '\n').take(2000).collect::<String>().trim().to_string() }
fn clean_name(s: &str) -> String { s.chars().filter(|c| !c.is_control()).take(40).collect::<String>().trim().to_string() }

pub struct Export<'a> { pub name: &'a str, pub notes: &'a str, pub voices: bool, pub runpod: bool, pub keys: bool, pub password: &'a str }

/// Writes a pack. The keys are only included with a password of 8+ characters.
pub fn export(st: &AppState, path: &str, o: &Export) -> Result<Value, String> {
    if o.keys && o.password.chars().count() < 8 { return Err("Choose a password of at least 8 characters to protect the RunPod keys.".into()); }
    let mut pack = json!({ "format": FORMAT, "version": 1, "name": clean_name(o.name), "notes": clean_notes(o.notes) });
    if o.voices {
        let vs: Vec<Value> = crate::voices::export_all(st, &[]).into_iter().map(|(n, ext, v, bytes)| json!({
            "name": n, "ext": ext, "speed": v["speed"], "seed": v["seed"], "language": v["language"], "instruct": v["instruct"], "ref_text": v["ref_text"], "profile_id": v["profile_id"],
            "consent": "speaker_permission", "audio_b64": B64.encode(bytes),
        })).collect();
        pack["voices"] = Value::Array(vs);
    }
    if o.runpod {
        let rp: Vec<Value> = SLOTS.iter().filter_map(|(slot, _, ep)| {
            let id = st.db.kv_get(ep).unwrap_or_default();
            if id.trim().is_empty() { return None; }
            let nm = st.db.kv_get(&format!("runpod_name_{slot}")).unwrap_or_default();
            Some(json!({ "slot": slot, "name": nm, "endpoint_id": id.trim(), "tts_endpoint_id": st.db.kv_get(tts_setting(slot)).unwrap_or_default().trim() }))
        }).collect();
        pack["runpod"] = Value::Array(rp);
        pack["runpod_active"] = json!(st.db.kv_get("runpod_active").unwrap_or_default());
        if o.keys {
            let mut ks = serde_json::Map::new();
            for (_, sec, _) in SLOTS { if let Some(k) = crate::secrets::get(sec) { ks.insert(sec.into(), json!(k)); } }
            pack["secrets"] = seal(o.password, Value::Object(ks).to_string().as_bytes())?;
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(&pack).map_err(|x| x.to_string())?).map_err(|x| x.to_string())?;
    Ok(json!({ "voices": pack["voices"].as_array().map(|a| a.len()).unwrap_or(0), "runpod": pack["runpod"].as_array().map(|a| a.len()).unwrap_or(0), "keys": pack["secrets"].is_object() }))
}

fn read(path: &str) -> Result<Value, String> {
    let m = std::fs::metadata(path).map_err(|x| x.to_string())?;
    if m.len() > 60 * 1024 * 1024 { return Err("This file is too large to be a profile pack.".into()); }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|x| x.to_string())?).map_err(|_| "This is not a profile pack (not valid JSON).".to_string())?;
    if v["format"] != FORMAT { return Err("This is not an AI Video Studio profile pack.".into()); }
    Ok(v)
}

/// What a pack contains and what it would replace, so the user can confirm before anything changes.
pub fn preview(st: &AppState, path: &str) -> Result<Value, String> {
    let p = read(path)?;
    let have = crate::voices::list(st);
    let voices: Vec<Value> = p["voices"].as_array().cloned().unwrap_or_default().iter().map(|v| json!({
        "name": v["name"], "seed": v["seed"], "speed": v["speed"], "instruct": v["instruct"], "replaces": have.iter().any(|x| x["name"] == v["name"]) })).collect();
    let runpod: Vec<Value> = p["runpod"].as_array().cloned().unwrap_or_default().iter().map(|r| {
        let slot = r["slot"].as_str().unwrap_or("1");
        let ep = SLOTS.iter().find(|k| k.0 == slot).map(|k| k.2).unwrap_or("runpod_endpoint_id");
        json!({ "slot": slot, "name": r["name"], "endpoint_id": r["endpoint_id"], "current_endpoint": st.db.kv_get(ep).unwrap_or_default() })
    }).collect();
    Ok(json!({ "name": p["name"], "notes": clean_notes(p["notes"].as_str().unwrap_or("")), "voices": voices, "runpod": runpod, "active": p["runpod_active"], "has_keys": p["secrets"].is_object() }))
}

pub struct Apply<'a> { pub voices: bool, pub runpod: bool, pub keys: bool, pub make_active: bool, pub password: &'a str }

pub fn apply(st: &AppState, path: &str, o: &Apply) -> Result<Value, String> {
    let p = read(path)?;
    // decrypt first: a wrong password must change nothing
    let keys: Option<Value> = if o.runpod && o.keys && p["secrets"].is_object() {
        Some(serde_json::from_slice(&open_sealed(o.password, &p["secrets"])?).map_err(|_| "The profile file is damaged.".to_string())?)
    } else { None };
    let (mut nv, mut nr) = (0, 0);
    if o.voices {
        for v in p["voices"].as_array().cloned().unwrap_or_default() {
            if v["consent"] != "speaker_permission" { continue; }
            let bytes = B64.decode(v["audio_b64"].as_str().unwrap_or("")).map_err(|_| "A voice sample in the file is damaged.".to_string())?;
            crate::voices::put(st, v["name"].as_str().unwrap_or(""), v["ext"].as_str().unwrap_or("mp3"), &bytes, &v)?;
            nv += 1;
        }
    }
    if o.runpod {
        for r in p["runpod"].as_array().cloned().unwrap_or_default() {
            let slot = r["slot"].as_str().unwrap_or("1");
            let Some((_, sec, ep)) = SLOTS.iter().find(|k| k.0 == slot) else { continue };
            let id = clean_id(r["endpoint_id"].as_str().unwrap_or(""));
            if id.is_empty() { continue; }
            st.db.kv_set(ep, &id).map_err(|x| x.to_string())?;
            st.db.kv_set(&format!("runpod_name_{slot}"), &clean_name(r["name"].as_str().unwrap_or(""))).map_err(|x| x.to_string())?;
            st.db.kv_set(tts_setting(slot), &clean_id(r["tts_endpoint_id"].as_str().unwrap_or(""))).map_err(|x| x.to_string())?;
            if let Some(k) = keys.as_ref().and_then(|k| k[*sec].as_str()) { crate::secrets::set(sec, k).map_err(|x| x.to_string())?; }
            nr += 1;
        }
        if o.make_active && nr > 0 {
            let a = if p["runpod_active"].as_str() == Some("2") { "2" } else { "1" };
            st.db.kv_set("runpod_active", a).map_err(|x| x.to_string())?;
            crate::runpod::instance_changed();
        }
    }
    Ok(json!({ "voices": nv, "runpod": nr, "keys": keys.is_some() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sealed_keys_open_only_with_the_password() {
        let s = seal("correct horse", b"{\"runpod_api_key\":\"abc\"}").unwrap();
        assert_eq!(open_sealed("correct horse", &s).unwrap(), b"{\"runpod_api_key\":\"abc\"}");
        assert!(open_sealed("wrong", &s).is_err());
        assert!(!s.to_string().contains("abc"));
    }
}
