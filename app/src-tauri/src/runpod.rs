//! RunPod Serverless endpoint: wake up / put to sleep, status, auto-sleep.
//!
//! The app has no rented cards. One RunPod Serverless ComfyUI endpoint (queue type, models on a network volume) does the
//! work and is billed per second only while a job runs. "Awake" and "asleep" are the endpoint's `workers.max` (1 / 0).
//!
//! Auto-sleep (max = 0) is a deliberate safety net. We observed that with max = 1 the endpoint scales to zero by itself and
//! the health API shows 0 workers, but the worker list still showed 3 IDLE workers after each job, and billing for those was
//! not confirmed. With max = 0 nothing can ever run, so nothing can ever bill. Do not remove it without checking the bill.
//!
//! REST contract, read from the OpenAPI document https://api.runpod.io/v2/openapi.json (not from memory):
//!   GET   https://api.runpod.io/v2/serverless/{id}  -> Endpoint object, `workers.max` is the ceiling       (scope serverless:read)
//!   PATCH https://api.runpod.io/v2/serverless/{id}  body {"workers":{"min":0,"max":N}}, partial update       (scope serverless:write)
//!   Authorization: Bearer <api key>. "Scaling and worker-bound settings are applied to the autoscaler promptly", but the
//!   job runtime (api.runpod.ai/v2/{id}/run) keeps answering 409 ENDPOINT_PAUSED for ~10-30 s after max is raised, so the
//!   job submit retries with backoff (see `retry_delay`).
//! Job submit / status / cancel / health use the other host, https://api.runpod.ai/v2/{id}/..., in hosted.rs.
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::AppState;

/// Management API base (note: `.io`, the job runtime is `.ai`).
pub const MGMT_BASE: &str = "https://api.runpod.io/v2/serverless";
/// `workers.max` while awake / asleep.
pub const AWAKE_MAX: u64 = 1;
pub const ASLEEP_MAX: u64 = 0;
/// Setting `runpod_sleep_min` default: minutes with an empty queue before the endpoint is put to sleep (0 = never).
pub const DEFAULT_SLEEP_MIN: u64 = 5;
/// How long a job submit keeps retrying 409 ENDPOINT_PAUSED after the wake-up.
pub const PAUSED_BUDGET: Duration = Duration::from_secs(90);

/// Endpoint ids are short alphanumeric strings; anything else must never reach a URL path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn endpoint_url(id: &str) -> String {
    format!("{MGMT_BASE}/{id}")
}

/// PATCH body that sets the worker ceiling (min stays 0, so nothing ever idles on our account).
pub fn workers_body(max: u64) -> Value {
    json!({ "workers": { "min": 0, "max": max } })
}

/// `workers.max` of a GET endpoint response.
pub fn parse_max(endpoint: &Value) -> Option<u64> {
    endpoint["workers"]["max"].as_u64()
}

/// "asleep" | "awake" | "working" for the status chip. `working` = a RunPod job is running right now.
pub fn state_name(max: u64, working: bool) -> &'static str {
    if max == 0 {
        "asleep"
    } else if working {
        "working"
    } else {
        "awake"
    }
}

/// The job runtime answers 409 while the endpoint is paused (max 0) or has just been woken up.
pub fn is_paused(status: u16) -> bool {
    status == 409
}

/// Wait before the next submit retry: 3, 5, 8, 10, 10, ... seconds.
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_secs([3u64, 5, 8, 10].get(attempt as usize).copied().unwrap_or(10))
}

/// Delay before retry number `attempt` (0-based), or None when waiting that long would pass the 90 s budget.
pub fn retry_delay(attempt: u32, waited: Duration) -> Option<Duration> {
    let d = backoff(attempt);
    (waited + d <= PAUSED_BUDGET).then_some(d)
}

/// `runpod_sleep_min` setting -> minutes (empty or bad value = default, 0 = never).
pub fn parse_sleep_min(v: Option<String>) -> u64 {
    v.and_then(|s| s.trim().parse().ok()).unwrap_or(DEFAULT_SLEEP_MIN)
}

/// True when the queue has been empty for at least `limit_min` minutes (0 = never).
pub fn sleep_due(idle: Duration, limit_min: u64) -> bool {
    limit_min > 0 && idle >= Duration::from_secs(limit_min * 60)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent("AI-Video-Studio").timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())
}

fn mgmt_err(status: u16, body: &Value) -> String {
    let msg = body["error"].as_str().or_else(|| body["message"].as_str()).map(String::from).unwrap_or_else(|| body.to_string());
    match status {
        401 => format!("RunPod: the API key was rejected. Check it in Settings. ({msg})"),
        403 => format!("RunPod: this API key may not change the endpoint. Create a key with Serverless read and write access. ({msg})"),
        404 => format!("RunPod: endpoint not found. Check the endpoint id in Settings. ({msg})"),
        _ => format!("RunPod: HTTP {status} {msg}"),
    }
}

async fn call(req: reqwest::RequestBuilder) -> Result<Value, String> {
    let resp = req.send().await.map_err(|e| format!("RunPod: {e}"))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        Ok(body)
    } else {
        Err(mgmt_err(status.as_u16(), &body))
    }
}

// ---- the current ceiling is cached for a few seconds so a status chip that polls does not hammer the REST API

static CACHE: Mutex<Option<(Instant, u64)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(10);

fn cache_set(max: u64) {
    *CACHE.lock().unwrap() = Some((Instant::now(), max));
}

/// Current `workers.max` straight from RunPod.
pub async fn get_max(key: &str, id: &str) -> Result<u64, String> {
    if !valid_id(id) {
        return Err("RunPod: the endpoint id looks wrong. Check it in Settings.".into());
    }
    let ep = call(client()?.get(endpoint_url(id)).bearer_auth(key)).await?;
    let max = parse_max(&ep).ok_or("RunPod: the endpoint answer has no worker settings.")?;
    cache_set(max);
    Ok(max)
}

pub async fn get_max_cached(key: &str, id: &str) -> Result<u64, String> {
    if let Some((t, m)) = *CACHE.lock().unwrap() {
        if t.elapsed() < CACHE_TTL {
            return Ok(m);
        }
    }
    get_max(key, id).await
}

pub async fn set_max(key: &str, id: &str, max: u64) -> Result<(), String> {
    if !valid_id(id) {
        return Err("RunPod: the endpoint id looks wrong. Check it in Settings.".into());
    }
    call(client()?.patch(endpoint_url(id)).bearer_auth(key).json(&workers_body(max))).await?;
    cache_set(max);
    Ok(())
}

// ---- session spend (jobs finished since the app started)

static SPEND_MICRO: AtomicU64 = AtomicU64::new(0);

pub fn add_spend(usd: f64) {
    if usd.is_finite() && usd > 0.0 {
        SPEND_MICRO.fetch_add((usd * 1e6).round() as u64, Ordering::Relaxed);
    }
}

pub fn session_usd() -> f64 {
    SPEND_MICRO.load(Ordering::Relaxed) as f64 / 1e6
}

/// Spend guard: the most USD one app session may spend on RunPod (setting runpod_cap_usd, default 5, 0 = no limit).
/// It can only be changed in the app's own Settings, never through the Claude/ZCode connection.
pub fn cap_usd(v: Option<String>) -> f64 {
    v.and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok()).filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(5.0)
}

pub fn over_cap(spent: f64, next: f64, cap: f64) -> bool {
    cap > 0.0 && spent + next > cap
}

// ---- wake / sleep

/// The last wake-up counts as activity, so a manual "Wake up" is not undone by auto-sleep before any job is sent.
static TOUCHED: Mutex<Option<Instant>> = Mutex::new(None);

fn touch() {
    *TOUCHED.lock().unwrap() = Some(Instant::now());
}

fn since_touch() -> Duration {
    TOUCHED.lock().unwrap().map(|t| t.elapsed()).unwrap_or(Duration::MAX)
}

static WAKE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ASLEEP_KNOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Credentials from the app's settings: (key, endpoint id).
pub fn creds(st: &AppState) -> Result<(String, String), String> {
    let key = crate::secrets::get("runpod_api_key").ok_or("The RunPod API key is missing. Add it in Settings.")?;
    let id = st.db.kv_get("runpod_endpoint_id").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or("The RunPod endpoint id is missing. Add it in Settings.")?;
    Ok((key, id))
}

/// "Wake up RunPod": raises the worker ceiling to 1 unless it is already above 0.
pub async fn wake(key: &str, id: &str) -> Result<String, String> {
    let _g = WAKE_LOCK.lock().await;
    ASLEEP_KNOWN.store(false, Ordering::Relaxed);
    touch();
    if get_max(key, id).await? > 0 {
        return Ok("RunPod is already awake.".into());
    }
    set_max(key, id, AWAKE_MAX).await?;
    crate::log::info("runpod: woke the endpoint (workers.max 0 -> 1)");
    Ok("RunPod is waking up. The first video can take a minute or two more while a worker starts.".into())
}

/// "Put RunPod to sleep": ceiling 0, so nothing can run or bill.
pub async fn sleep(key: &str, id: &str) -> Result<String, String> {
    let _g = WAKE_LOCK.lock().await;
    set_max(key, id, ASLEEP_MAX).await?;
    ASLEEP_KNOWN.store(true, Ordering::Relaxed);
    crate::log::info("runpod: put the endpoint to sleep (workers.max -> 0)");
    Ok("RunPod is asleep. Nothing can be billed now.".into())
}

/// Before a job is submitted: wakes the endpoint when it is asleep. A failing management call (for example a key that is
/// limited to running jobs) does not stop the job; the submit step then reports the problem if the endpoint really is paused.
pub async fn ensure_awake(key: &str, id: &str, say: &(dyn Fn(&str) + Sync)) {
    match get_max_cached(key, id).await {
        Ok(0) => {
            say("Waking up RunPod…");
            if let Err(e) = wake(key, id).await {
                crate::log::info(&format!("runpod: auto wake failed: {e}"));
            }
        }
        Ok(_) => ASLEEP_KNOWN.store(false, Ordering::Relaxed),
        Err(e) => crate::log::info(&format!("runpod: could not read the endpoint state, submitting anyway: {e}")),
    }
}

fn busy_statuses(s: &str) -> bool {
    matches!(s, "queued" | "routing" | "generating" | "downloading")
}

/// Jobs of RunPod modes that are in flight (`running_only`: not the ones still waiting in the queue).
fn runpod_jobs(st: &AppState, running_only: bool) -> usize {
    let (wfs, _) = st.load_workflows();
    st.db.jobs(None).map(|jobs| {
        jobs.iter().filter(|j| {
            let live = if running_only { matches!(j.status.as_str(), "routing" | "generating" | "downloading") } else { busy_statuses(&j.status) };
            // an open-source mode sent to RunPod runs as its RunPod twin
            let id = crate::workflows::effective_id(&wfs, &j.workflow_id, &crate::queue::job_provider(st, j));
            live && wfs.iter().any(|w| w.meta.id == id && w.meta.provider == "runpod")
        }).count()
    }).unwrap_or(0)
}

/// Everything the UI, the chat and the MCP tools show about RunPod.
pub async fn status(st: &AppState) -> Result<Value, String> {
    let (key, id) = creds(st)?;
    let max = get_max_cached(&key, &id).await?;
    let working = runpod_jobs(st, true) > 0;
    let rate = st.db.kv_get("runpod_gpu_dph").and_then(|s| s.trim().parse::<f64>().ok()).filter(|d| *d > 0.0).unwrap_or(crate::hosted::RUNPOD_DEFAULT_DPH);
    Ok(json!({
        "state": state_name(max, working),
        "workers_max": max,
        "endpoint_id": id,
        "jobs_waiting": runpod_jobs(st, false),
        "session_usd": (session_usd() * 1000.0).round() / 1000.0,
        "usd_per_hour_while_working": rate,
        "auto_sleep_min": parse_sleep_min(st.db.kv_get("runpod_sleep_min")),
        "note": "Billed per second only while a job runs. asleep = max workers 0 (nothing can run or bill); it wakes up by itself when a job is sent.",
    }))
}

/// Safety net: puts the endpoint to sleep when the queue has been empty for `runpod_sleep_min` minutes (default 5, 0 = never).
/// Runs for the whole life of the app. The endpoint is only asked while it is not known to be asleep.
pub async fn auto_sleep_loop(app: AppHandle) {
    let mut last_active = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let st = app.state::<AppState>();
        let busy = st.db.jobs(None).map(|j| j.iter().any(|j| busy_statuses(&j.status))).unwrap_or(false);
        if busy {
            last_active = Instant::now();
            continue;
        }
        let limit = parse_sleep_min(st.db.kv_get("runpod_sleep_min"));
        if !sleep_due(last_active.elapsed().min(since_touch()), limit) || ASLEEP_KNOWN.load(Ordering::Relaxed) {
            continue;
        }
        let Ok((key, id)) = creds(&st) else { continue };
        match get_max(&key, &id).await {
            Ok(0) => ASLEEP_KNOWN.store(true, Ordering::Relaxed),
            Ok(_) => {
                crate::log::info(&format!("runpod: queue empty for {limit} min, putting the endpoint to sleep"));
                if sleep(&key, &id).await.is_ok() {
                    use tauri::Emitter;
                    let _ = app.emit("runpod-slept", ());
                }
            }
            Err(e) => crate::log::info(&format!("runpod: auto-sleep check failed: {e}")),
        }
    }
}

/// App exit: sleeps the endpoint when `runpod_sleep_on_exit` is not "0". Waits at most 10 s.
pub async fn sleep_on_exit(st: &AppState) {
    if st.db.kv_get("runpod_sleep_on_exit").as_deref() == Some("0") {
        return;
    }
    let Ok((key, id)) = creds(st) else { return };
    let _ = tokio::time::timeout(Duration::from_secs(10), async {
        if matches!(get_max(&key, &id).await, Ok(m) if m > 0) {
            let _ = sleep(&key, &id).await;
        }
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_and_sleep_requests_are_built_from_the_spec() {
        assert_eq!(endpoint_url("abc123"), "https://api.runpod.io/v2/serverless/abc123");
        assert_eq!(workers_body(AWAKE_MAX), json!({"workers": {"min": 0, "max": 1}}));
        assert_eq!(workers_body(ASLEEP_MAX), json!({"workers": {"min": 0, "max": 0}}));
        assert_eq!(parse_max(&json!({"id": "x", "workers": {"min": 0, "max": 3, "idleTimeout": 5}})), Some(3));
        assert_eq!(parse_max(&json!({"id": "x"})), None);
    }

    #[test]
    fn endpoint_ids_cannot_escape_the_url() {
        assert!(valid_id("4m7x2k9q"));
        assert!(valid_id("ep_abc-123"));
        for bad in ["", "a/b", "../x", "a b", "a?x=1", "a#b", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn state_follows_max_and_activity() {
        assert_eq!(state_name(0, false), "asleep");
        assert_eq!(state_name(0, true), "asleep");
        assert_eq!(state_name(1, false), "awake");
        assert_eq!(state_name(2, true), "working");
    }

    #[test]
    fn paused_409_is_retried_with_backoff_for_90_seconds() {
        assert!(is_paused(409));
        assert!(!is_paused(401) && !is_paused(404) && !is_paused(500));
        let mut waited = Duration::ZERO;
        let mut n = 0u32;
        let mut delays = vec![];
        while let Some(d) = retry_delay(n, waited) {
            delays.push(d.as_secs());
            waited += d;
            n += 1;
        }
        assert_eq!(&delays[..5], &[3, 5, 8, 10, 10]);
        assert!(waited <= PAUSED_BUDGET);
        assert!(waited >= Duration::from_secs(80), "uses most of the budget: {waited:?}");
        assert!(retry_delay(0, Duration::from_secs(89)).is_none());
        assert_eq!(retry_delay(0, Duration::ZERO), Some(Duration::from_secs(3)));
        assert_eq!(backoff(50), Duration::from_secs(10));
    }

    #[test]
    fn auto_sleep_settings() {
        assert_eq!(parse_sleep_min(None), 5);
        assert_eq!(parse_sleep_min(Some("".into())), 5);
        assert_eq!(parse_sleep_min(Some("abc".into())), 5);
        assert_eq!(parse_sleep_min(Some("0".into())), 0);
        assert_eq!(parse_sleep_min(Some(" 15 ".into())), 15);
        assert!(!sleep_due(Duration::from_secs(3600), 0));
        assert!(!sleep_due(Duration::from_secs(299), 5));
        assert!(sleep_due(Duration::from_secs(300), 5));
    }

    #[test]
    fn spend_cap_blocks_only_above_the_limit() {
        assert_eq!(cap_usd(None), 5.0);
        assert_eq!(cap_usd(Some("2,5".into())), 2.5);
        assert_eq!(cap_usd(Some("abc".into())), 5.0);
        assert!(!over_cap(4.0, 0.9, 5.0));
        assert!(over_cap(4.5, 0.9, 5.0));
        assert!(!over_cap(99.0, 5.0, 0.0));
    }

    #[test]
    fn spend_is_summed_and_ignores_junk() {
        let before = session_usd();
        add_spend(0.0625);
        add_spend(f64::NAN);
        add_spend(-3.0);
        assert!((session_usd() - before - 0.0625).abs() < 1e-6);
    }

    #[test]
    fn errors_name_the_fix() {
        assert!(mgmt_err(403, &json!({})).contains("read and write"));
        assert!(mgmt_err(401, &json!({"error": "bad"})).contains("rejected"));
        assert!(mgmt_err(404, &json!({})).contains("endpoint id"));
    }
}
