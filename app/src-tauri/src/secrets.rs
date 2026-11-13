use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

const SERVICE: &str = "AIVideoStudio";

/// Names the UI is allowed to store. Values never travel back to the UI.
pub const NAMES: &[&str] = &[
    // legacy: the app no longer uses Vast.ai; the name stays so an old saved key can still be read, replaced or cleared
    "vast_api_key",
    "anthropic_api_key",
    "gemini_api_key",
    "openai_api_key",
    "glm_api_key",
    "deepseek_api_key",
    "openrouter_api_key",
    "fal_key",
    "replicate_token",
    "runpod_api_key",
    "runpod_api_key_2",
];

fn entry(name: &str) -> Result<keyring::Entry> {
    if !NAMES.contains(&name) {
        return Err(anyhow!("unknown secret {name}"));
    }
    Ok(keyring::Entry::new(SERVICE, name)?)
}

pub fn get(name: &str) -> Option<String> {
    entry(name).ok()?.get_password().ok().filter(|s| !s.trim().is_empty())
}

pub fn set(name: &str, value: &str) -> Result<()> {
    let e = entry(name)?;
    let v = value.trim();
    if v.is_empty() {
        let _ = e.delete_credential();
        return Ok(());
    }
    e.set_password(v)?;
    Ok(())
}

pub fn status() -> BTreeMap<String, bool> {
    NAMES.iter().map(|n| (n.to_string(), get(n).is_some())).collect()
}
