//! Minimal Vast.ai Serverless client (HTTP only, no Python SDK).
//! Protocol taken from docs.vast.ai and the `vastai` 1.9.1 SDK:
//!   1. POST https://run.vast.ai/route/            -> worker `url` + signed auth fields (or "no worker yet")
//!   2. POST <url>/generate/sync {auth_data, payload} -> ComfyUI result with pre-signed output URLs
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

fn run_url() -> String {
    std::env::var("AIVS_VAST_RUN_URL").unwrap_or_else(|_| "https://run.vast.ai".into())
}
fn console_url() -> String {
    std::env::var("AIVS_VAST_CONSOLE_URL").unwrap_or_else(|_| "https://console.vast.ai".into())
}
const ROOT_CERT_URL: &str = "https://console.vast.ai/static/jvastai_root.cer";

pub struct Vast {
    api: reqwest::Client,
    worker: tokio::sync::Mutex<Option<reqwest::Client>>,
}

pub enum Route {
    Ready { request_idx: i64, body: Value },
    Waiting { request_idx: i64, status: String },
}

#[derive(Serialize, Clone, Debug)]
pub struct EndpointInfo {
    pub id: i64,
    pub name: String,
    pub state: String,
    pub cold_workers: i64,
    pub min_load: f64,
    pub max_workers: i64,
    pub scales_to_zero: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct InstanceInfo {
    pub id: i64,
    pub gpu: String,
    pub status: String,
    pub usd_per_hour: f64,
    pub minutes: f64,
    pub cost_so_far: f64,
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct GpuOffer {
    pub name: String,
    pub vram_gb: f64,
    pub min_price: f64,
    pub median_price: f64,
    pub offers: usize,
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Vast {
    pub fn new() -> Vast {
        let api = reqwest::Client::builder()
            .user_agent("AIVideoStudio/0.1")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("http client");
        Vast { api, worker: tokio::sync::Mutex::new(None) }
    }

    /// Worker URLs are signed by the Vast root CA; fetch it once and trust it for worker calls.
    async fn worker_client(&self) -> Result<reqwest::Client, String> {
        let mut g = self.worker.lock().await;
        if let Some(c) = g.as_ref() {
            return Ok(c.clone());
        }
        let base = || {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(1800))
        };
        // Try with the Vast root CA; if it can't be loaded (rustls is strict about it), use the default roots.
        let mut with_ca = None;
        if let Ok(r) = self.api.get(ROOT_CERT_URL).send().await {
            if let Ok(bytes) = r.bytes().await {
                if let Ok(c) = reqwest::Certificate::from_der(&bytes).or_else(|_| reqwest::Certificate::from_pem(&bytes)) {
                    with_ca = base().add_root_certificate(c).build().ok();
                }
            }
        }
        let c = match with_ca {
            Some(c) => c,
            None => base().build().map_err(err)?,
        };
        *g = Some(c.clone());
        Ok(c)
    }

    pub async fn route(&self, key: &str, endpoint: &str, request_idx: i64) -> Result<Route, String> {
        let r = self
            .api
            .post(format!("{}/route/", run_url()))
            .bearer_auth(key)
            .json(&json!({
                "endpoint": endpoint, "api_key": key, "cost": 100,
                "request_idx": request_idx, "replay_timeout": 60.0
            }))
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(err)?;
        let code = r.status();
        let text = r.text().await.map_err(err)?;
        crate::log::on_change("route", "INFO", &format!("route {endpoint}: HTTP {code} {}", text.chars().take(200).collect::<String>()), &format!("{code}{}", text.chars().take(60).collect::<String>()));
        if code.as_u16() == 401 || code.as_u16() == 403 {
            return Err("Vast: invalid API key (401/403)".into());
        }
        if !code.is_success() {
            return Err(format!("Vast route HTTP {code}: {}", text.chars().take(300).collect::<String>()));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("route JSON: {e}"))?;
        // Bad key / unknown endpoint comes back as HTTP 200 with an error message.
        if let Some(m) = v.get("error_msg").and_then(|m| m.as_str()) {
            return Err(format!("Vast: {m} (check the API key and the endpoint name)"));
        }
        let idx = v.get("request_idx").and_then(|x| x.as_i64()).unwrap_or(request_idx);
        if v.get("url").and_then(|u| u.as_str()).is_some() {
            Ok(Route::Ready { request_idx: idx, body: v })
        } else {
            let status = v
                .get("status")
                .map(|s| s.as_str().map(String::from).unwrap_or_else(|| s.to_string()))
                .unwrap_or_default();
            Ok(Route::Waiting { request_idx: idx, status })
        }
    }

    pub async fn generate(
        &self,
        key: &str,
        auth_data: &Value,
        workflow: &Value,
        request_id: &str,
    ) -> Result<Value, String> {
        let url = auth_data["url"].as_str().ok_or("route without url")?;
        let body = json!({
            "auth_data": auth_data,
            "session_id": Value::Null,
            "payload": { "input": { "request_id": request_id, "workflow_json": workflow } }
        });
        let endpoint = format!("{}/generate/sync", url.trim_end_matches('/'));
        loop {
            let c = self.worker_client().await?;
            let t0 = std::time::Instant::now();
            crate::log::info(&format!("worker generate → {endpoint} (request {request_id})"));
            // the account key is only ever sent over a verified TLS connection (there is no insecure retry)
            match c.post(&endpoint).json(&body).bearer_auth(key).send().await {
                Ok(r) => {
                    let code = r.status();
                    let text = r.text().await.map_err(err)?;
                    crate::log::info(&format!("worker generate ← HTTP {code} in {:.1}s, {} bytes: {}", t0.elapsed().as_secs_f32(), text.len(), text.chars().take(300).collect::<String>()));
                    if !code.is_success() {
                        return Err(format!("Worker HTTP {code}: {}", text.chars().take(400).collect::<String>()));
                    }
                    return serde_json::from_str(&text).map_err(|e| format!("worker JSON: {e}"));
                }
                Err(e) => {
                    crate::log::error(&format!("worker generate failed: {e:?}"));
                    return Err(format!("Worker: {e}"));
                }
            }
        }
    }

    async fn console_get(&self, key: &str, path: &str) -> Result<Value, String> {
        let r = self.api.get(format!("{}{path}", console_url())).bearer_auth(key).send().await.map_err(err)?;
        let code = r.status();
        let text = r.text().await.map_err(err)?;
        crate::log::info(&format!("vast GET {path} → HTTP {code}{}", if code.is_success() { String::new() } else { format!(": {}", text.chars().take(200).collect::<String>()) }));
        if code.as_u16() == 401 || code.as_u16() == 403 {
            return Err("Vast: invalid API key".into());
        }
        if !code.is_success() {
            return Err(format!("Vast HTTP {code}: {}", text.chars().take(300).collect::<String>()));
        }
        serde_json::from_str(&text).map_err(|e| format!("JSON: {e}"))
    }

    async fn console_send(&self, method: reqwest::Method, key: &str, path: &str, body: Value) -> Result<Value, String> {
        let method_name = method.to_string();
        let r = self
            .api
            .request(method, format!("{}{path}", console_url()))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(err)?;
        let code = r.status();
        let text = r.text().await.map_err(err)?;
        crate::log::info(&format!("vast {} {path} → HTTP {code}{}", method_name, if code.is_success() { String::new() } else { format!(": {}", text.chars().take(200).collect::<String>()) }));
        if !code.is_success() {
            return Err(format!("Vast HTTP {code}: {}", text.chars().take(300).collect::<String>()));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// Newest copy of Vast's own "Wan 2.2 14B t2v (Serverless)" template (creator 62897).
    pub async fn official_template(&self, key: &str) -> Result<String, String> {
        let q = format!(
            "/api/v0/template/?select_cols={}&select_filters={}",
            urlenc(&json!(["*"]).to_string()),
            urlenc(&json!({"name": {"eq": "Wan 2.2 14B t2v (Serverless)"}, "creator_id": {"eq": 62897}}).to_string())
        );
        let v = self.console_get(key, &q).await?;
        v["templates"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t["deleted_at"].is_null())
            .max_by(|a, b| a["created_at"].as_f64().partial_cmp(&b["created_at"].as_f64()).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(|t| t["hash_id"].as_str().map(String::from))
            .ok_or_else(|| "The official template \"Wan 2.2 14B t2v (Serverless)\" was not found on Vast.".to_string())
    }

    /// Scale-to-zero endpoint: no worker is kept running while idle.
    pub async fn create_endpoint(&self, key: &str, name: &str) -> Result<(), String> {
        self.console_send(
            reqwest::Method::POST,
            key,
            "/api/v0/endptjobs/",
            json!({"endpoint_name": name, "min_load": 0, "target_util": 0.9, "cold_mult": 3.0, "cold_workers": 0, "max_workers": 2}),
        )
        .await
        .map(|_| ())
    }

    pub async fn create_workergroup(&self, key: &str, endpoint_id: i64, name: &str, template_hash: &str, search_params: &str, gpu_ram: f64) -> Result<(), String> {
        self.console_send(
            reqwest::Method::POST,
            key,
            "/api/v0/workergroups/",
            json!({
                "endpoint_name": name, "endpoint_id": endpoint_id, "template_hash": template_hash,
                "search_params": search_params, "gpu_ram": gpu_ram,
                "min_load": 0, "target_util": 0.9, "cold_mult": 3.0, "cold_workers": 0, "max_workers": 2, "test_workers": 1
            }),
        )
        .await
        .map(|_| ())
    }

    pub async fn endpoints(&self, key: &str) -> Result<Vec<EndpointInfo>, String> {
        let v = self.console_get(key, "/api/v0/endptjobs/").await?;
        let arr = v.get("results").cloned().unwrap_or(v);
        Ok(arr
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| {
                        let cold = e["cold_workers"].as_i64().unwrap_or(0);
                        let min_load = e["min_load"].as_f64().unwrap_or(0.0);
                        EndpointInfo {
                            id: e["id"].as_i64().unwrap_or(0),
                            name: e["endpoint_name"].as_str().unwrap_or("").into(),
                            state: e["endpoint_state"].as_str().unwrap_or("").into(),
                            cold_workers: cold,
                            min_load,
                            max_workers: e["max_workers"].as_i64().unwrap_or(0),
                            scales_to_zero: cold == 0 && min_load == 0.0,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Cards currently rented on this account (running, loading, stopped). Each one is billed.
    pub async fn instances(&self, key: &str) -> Result<Vec<InstanceInfo>, String> {
        let v = self.console_get(key, "/api/v1/instances/").await?;
        let arr = v.get("instances").cloned().unwrap_or(v);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        Ok(arr
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|x| {
                        let dph = x["dph_total"].as_f64().unwrap_or(0.0);
                        let mins = x["start_date"].as_f64().map(|s| ((now - s) / 60.0).max(0.0)).unwrap_or(0.0);
                        InstanceInfo {
                            id: x["id"].as_i64().unwrap_or(0),
                            gpu: x["gpu_name"].as_str().unwrap_or("?").into(),
                            status: x["actual_status"].as_str().or(x["cur_state"].as_str()).unwrap_or("?").into(),
                            usd_per_hour: dph,
                            minutes: mins,
                            cost_so_far: dph * mins / 60.0,
                            label: x["label"].as_str().unwrap_or("").into(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Destroys a rented card: billing stops at once (stopping it would still bill the disk).
    pub async fn destroy_instance(&self, key: &str, id: i64) -> Result<(), String> {
        self.console_send(reqwest::Method::DELETE, key, &format!("/api/v0/instances/{id}/"), json!({})).await.map(|_| ())
    }

    pub async fn scale_to_zero(&self, key: &str, endpoint_id: i64) -> Result<(), String> {
        self.console_send(
            reqwest::Method::PUT,
            key,
            &format!("/api/v0/endptjobs/{endpoint_id}/"),
            json!({ "cold_workers": 0, "min_load": 0 }),
        )
        .await
        .map(|_| ())
    }

    /// Returns the workergroup (id, search_params) that belongs to the endpoint.
    pub async fn workergroup(&self, key: &str, endpoint_id: i64, endpoint_name: &str) -> Result<Option<(i64, String)>, String> {
        let v = self.console_get(key, "/api/v0/workergroups/").await?;
        let arr = v.get("results").cloned().unwrap_or(v);
        for g in arr.as_array().cloned().unwrap_or_default() {
            let by_id = g["endpoint_id"].as_i64() == Some(endpoint_id);
            let by_name = g["endpoint_name"].as_str() == Some(endpoint_name);
            if by_id || by_name {
                let sp = match &g["search_params"] {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    o => o.to_string(),
                };
                return Ok(Some((g["id"].as_i64().unwrap_or(0), sp)));
            }
        }
        Ok(None)
    }

    pub async fn set_search_params(&self, key: &str, endpoint_id: i64, workergroup_id: i64, search_params: &str, gpu_ram_gb: f64) -> Result<(), String> {
        self.console_send(
            reqwest::Method::PUT,
            key,
            &format!("/api/v0/workergroups/{workergroup_id}/"),
            json!({ "endpoint_id": endpoint_id, "search_params": search_params, "gpu_ram": gpu_ram_gb }),
        )
        .await
        .map(|_| ())
    }


    /// Full workergroup JSON for the endpoint (contains template_hash, search_params, ...).
    pub async fn workergroup_json(&self, key: &str, endpoint_id: i64, endpoint_name: &str) -> Result<Option<Value>, String> {
        let v = self.console_get(key, "/api/v0/workergroups/").await?;
        let arr = v.get("results").cloned().unwrap_or(v);
        Ok(arr.as_array().cloned().unwrap_or_default().into_iter().find(|g| {
            g["endpoint_id"].as_i64() == Some(endpoint_id) || g["endpoint_name"].as_str() == Some(endpoint_name)
        }))
    }

    pub async fn template_by_hash(&self, key: &str, hash: &str) -> Result<Value, String> {
        let q = format!(
            "/api/v0/template/?select_cols={}&select_filters={}",
            urlenc(&json!(["*"]).to_string()),
            urlenc(&json!({"hash_id": {"eq": hash}}).to_string())
        );
        let v = self.console_get(key, &q).await?;
        v["templates"].as_array().and_then(|a| a.first().cloned()).ok_or_else(|| format!("Template {hash} not found"))
    }

    /// Creates a new private template version and returns its hash_id.
    pub async fn create_template(&self, key: &str, body: Value) -> Result<String, String> {
        let r = self.console_send(reqwest::Method::POST, key, "/api/v0/template/", body).await?;
        crate::provision::find_hash(&r).ok_or_else(|| format!("Vast returned no hash_id: {r}"))
    }

    pub async fn set_template_hash(&self, key: &str, endpoint_id: i64, workergroup_id: i64, hash: &str) -> Result<(), String> {
        self.console_send(
            reqwest::Method::PUT,
            key,
            &format!("/api/v0/workergroups/{workergroup_id}/"),
            json!({ "endpoint_id": endpoint_id, "template_hash": hash }),
        )
        .await
        .map(|_| ())
    }

    /// Cheapest suitable machine for one GPU model: (offer id, $/hour). Needs a big disk and a fast download link.
    pub async fn find_offers(&self, key: &str, gpu: &str, disk_gb: f64) -> Result<Vec<(i64, f64)>, String> {
        let v = self
            .console_send(
                reqwest::Method::POST,
                key,
                "/api/v0/bundles/",
                json!({
                    "limit": 40, "type": "ondemand",
                    "verified": {"eq": true}, "rentable": {"eq": true}, "rented": {"eq": false},
                    "num_gpus": {"eq": 1}, "gpu_name": {"eq": gpu},
                    "disk_space": {"gte": disk_gb},
                    "inet_down": {"gte": 400},
                    "reliability": {"gte": 0.97},
                    "compute_cap": {"gte": 750}, "cuda_max_good": {"gte": 12.9},
                    "direct_port_count": {"gte": 2},
                    "order": [["dph_total", "asc"]]
                }),
            )
            .await?;
        // not in China: HuggingFace (where the models come from) is blocked or very slow there
        Ok(v["offers"].as_array().cloned().unwrap_or_default().iter()
            .filter(|o| !o["geolocation"].as_str().unwrap_or("").ends_with("CN"))
            .filter_map(|o| Some((o["id"].as_i64()?, o["dph_total"].as_f64()?)))
            .collect())
    }

    /// Rents the machine and starts the image there. Returns the new instance id.
    pub async fn create_instance(&self, key: &str, offer: i64, body: Value) -> Result<i64, String> {
        let v = self.console_send(reqwest::Method::PUT, key, &format!("/api/v0/asks/{offer}/"), body).await?;
        v["new_contract"].as_i64().ok_or_else(|| format!("Vast did not return an instance id: {}", v.to_string().chars().take(200).collect::<String>()))
    }

    /// Full JSON of every instance of the account (ports, ip, status, label).
    pub async fn instances_raw(&self, key: &str) -> Result<Vec<Value>, String> {
        let v = self.console_get(key, "/api/v1/instances/").await?;
        Ok(v.get("instances").cloned().unwrap_or(v).as_array().cloned().unwrap_or_default())
    }

    /// Last lines of the instance's log (only works while the container exists).
    pub async fn instance_status_msg(&self, key: &str, id: i64) -> String {
        self.instances_raw(key).await.unwrap_or_default().iter().find(|i| i["id"].as_i64() == Some(id)).and_then(|i| i["status_msg"].as_str().map(String::from)).unwrap_or_default()
    }

    pub async fn instance_logs(&self, key: &str, id: i64) -> String {
        let Ok(v) = self.console_send(reqwest::Method::PUT, key, &format!("/api/v0/instances/request_logs/{id}/"), json!({"tail": "60"})).await else { return String::new() };
        let Some(url) = v["result_url"].as_str() else { return String::new() };
        tokio::time::sleep(Duration::from_secs(3)).await;
        match self.api.get(url).send().await {
            Ok(r) => r.text().await.unwrap_or_default().chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect(),
            Err(_) => String::new(),
        }
    }

    /// Live marketplace prices, grouped by GPU model ($/hour, single-GPU machines).
    pub async fn gpu_offers(&self, key: &str, min_vram_gb: f64) -> Result<Vec<GpuOffer>, String> {
        let v = self
            .console_send(
                reqwest::Method::POST,
                key,
                "/api/v0/bundles/",
                json!({
                    "limit": 600, "type": "ondemand",
                    "verified": {"eq": true}, "rentable": {"eq": true}, "rented": {"eq": false},
                    "num_gpus": {"eq": 1},
                    "gpu_ram": {"gte": (min_vram_gb * 1000.0) as i64},
                    "reliability": {"gte": 0.95},
                    "order": [["dph_total", "asc"]]
                }),
            )
            .await?;
        let mut by: BTreeMap<String, (f64, Vec<f64>)> = BTreeMap::new();
        for o in v["offers"].as_array().cloned().unwrap_or_default() {
            let (Some(n), Some(p)) = (o["gpu_name"].as_str(), o["dph_total"].as_f64()) else { continue };
            let e = by.entry(n.to_string()).or_insert((o["gpu_ram"].as_f64().unwrap_or(0.0) / 1000.0, vec![]));
            e.1.push(p);
        }
        let mut out: Vec<GpuOffer> = by
            .into_iter()
            .map(|(name, (vram, mut ps))| {
                ps.sort_by(|a, b| a.partial_cmp(b).unwrap());
                GpuOffer { name, vram_gb: vram.round(), min_price: ps[0], median_price: ps[ps.len() / 2], offers: ps.len() }
            })
            .collect();
        out.sort_by(|a, b| a.min_price.partial_cmp(&b.min_price).unwrap());
        Ok(out)
    }
}

/// Merge `gpu_name in [...]` into an existing search_params string, preserving the other filters.
pub fn merge_gpu_filter(existing: &str, gpus: &[String]) -> String {
    let trimmed = existing.trim();
    let mut parts: Vec<String> = vec![];
    if trimmed.starts_with('{') {
        if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(trimmed) {
            for (k, v) in m {
                if k == "gpu_name" { continue; }
                match v.get("eq") {
                    Some(x) => parts.push(format!("{k}={}", x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))),
                    None => {}
                }
            }
        }
    } else {
        // tokens like "a=b" or "gpu_name in [x,y]" — drop any gpu_name clause
        let cleaned = regex_free_strip_gpu(trimmed);
        if !cleaned.is_empty() { parts.push(cleaned); }
    }
    if parts.is_empty() {
        parts = vec!["verified=true".into(), "rentable=true".into(), "rented=false".into()];
    }
    if !gpus.is_empty() {
        parts.push(format!("gpu_name in [{}]", gpus.join(",")));
    }
    parts.join(" ")
}

fn regex_free_strip_gpu(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(i) = rest.find("gpu_name") {
            out.push_str(&rest[..i]);
            let after = &rest[i..];
            // skip "gpu_name <op> [ ... ]" or "gpu_name=value"
            if let Some(b) = after.find(']').filter(|&b| after[..b].contains('[')) {
                rest = &after[b + 1..];
            } else {
                rest = after.split_once(char::is_whitespace).map(|x| x.1).unwrap_or("");
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_filter_replaces_old_clause() {
        let r = merge_gpu_filter("verified=true rentable=true gpu_name in [RTX_3090] rented=false", &["RTX_4090".into(), "RTX_5090".into()]);
        assert_eq!(r, "verified=true rentable=true rented=false gpu_name in [RTX_4090,RTX_5090]");
        assert_eq!(merge_gpu_filter("", &[]), "verified=true rentable=true rented=false");
    }
}
