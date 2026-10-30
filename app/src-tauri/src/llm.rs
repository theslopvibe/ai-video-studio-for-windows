//! Non-Anthropic prompt-improvement providers: Google Gemini and any OpenAI-compatible API
//! (OpenAI, OpenRouter, Groq, Together, local Ollama/LM Studio ... via a custom base URL).
use serde_json::{json, Value};
use std::time::Duration;

pub struct Out {
    pub text: String,
    pub tokens: Option<u64>,
}

fn client(secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(secs)).build().map_err(|e| e.to_string())
}

async fn read(r: reqwest::Response, who: &str) -> Result<Value, String> {
    let code = r.status();
    let text = r.text().await.map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !code.is_success() {
        let m = v["error"]["message"].as_str().map(String::from).unwrap_or_else(|| text.chars().take(200).collect());
        return Err(format!("{who} {code}: {m}"));
    }
    Ok(v)
}

pub async fn gemini(key: &str, model: &str, system: &str, input: &str, max_tokens: u32) -> Result<Out, String> {
    let mut body = json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": input}]}],
        "generationConfig": {"maxOutputTokens": max_tokens, "temperature": 0.8}
    });
    // 2.5 "flash" models think by default; thinking only burns tokens for this task.
    if model.contains("flash") {
        body["generationConfig"]["thinkingConfig"] = json!({"thinkingBudget": 0});
    }
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let r = client(60)?.post(url).header("x-goog-api-key", key).json(&body).send().await.map_err(|e| e.to_string())?;
    let v = read(r, "Gemini").await?;
    let text = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(format!("Gemini: empty answer ({})", v["candidates"][0]["finishReason"].as_str().unwrap_or("?")));
    }
    Ok(Out { text: text.trim().trim_matches('"').into(), tokens: v["usageMetadata"]["totalTokenCount"].as_u64() })
}

pub async fn openai(base: &str, key: &str, model: &str, system: &str, input: &str, max_tokens: u32) -> Result<Out, String> {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let mut rq = client(60)?.post(url).json(&json!({
        "model": model, "max_tokens": max_tokens, "temperature": 0.8,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": input}]
    }));
    if !key.is_empty() {
        rq = rq.bearer_auth(key);
    }
    let v = read(rq.send().await.map_err(|e| e.to_string())?, "OpenAI-compatible").await?;
    let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err("OpenAI-compatible: κενή απάντηση".into());
    }
    Ok(Out { text: text.trim_matches('"').into(), tokens: v["usage"]["total_tokens"].as_u64() })
}
