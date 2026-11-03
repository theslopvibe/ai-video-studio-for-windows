//! One rented Vast card ("instance") that runs ComfyUI for this app: created on demand, closed by the
//! auto-close loop / the Close button / the chat. Replaces the serverless route, which never produced a ready worker.
use crate::{provision, secrets, AppState};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const LABEL: &str = "aivs-card";
const WAN_PACK: &str = "wan22-i2v";
const FLUX_PACK: &str = "flux3-klein";
/// Cards with 16 GB or more, tried by price, for models that are small (FLUX and similar).
const CHEAP_GPUS: &[&str] = &["RTX 3090", "RTX 4080", "RTX 4080S", "RTX 4070S Ti", "RTX 4090"];
const DISK_GB: f64 = 110.0;
const IMAGE: &str = "vastai/comfy:v0.43.0-cuda-12.9-py312";

/// Held only while deciding "reuse a card / rent a new one", so two jobs never rent more cards than the limit.
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// card id -> (jobs using it, warm-ups waiting for it). A card used by a job is not given to a second job.
static LEASES: std::sync::Mutex<Vec<(i64, u32, u32)>> = std::sync::Mutex::new(Vec::new());
static LAST_USED: std::sync::Mutex<Vec<(i64, Instant)>> = std::sync::Mutex::new(Vec::new());
/// Cards whose ComfyUI has answered (set by `ensure` and by the watchdog).
static READY: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());

/// What a call to `ensure` needs.
#[derive(Clone, Copy, Default)]
pub struct Want {
    /// Picture-only job: a cheap FLUX-only card is enough (setting `image_card`).
    pub image: bool,
    /// Rent one more card even if an idle one exists ("two cards at once").
    pub extra: bool,
    /// A generation job (false = only warming a card up).
    pub job: bool,
}

pub struct Lease {
    id: i64,
    job: bool,
}

impl Lease {
    fn new(job: bool) -> Self {
        Lease { id: 0, job }
    }
    fn set(&mut self, id: i64) {
        self.release();
        self.id = id;
        if let Ok(mut l) = LEASES.lock() {
            match l.iter_mut().find(|x| x.0 == id) {
                Some(x) => if self.job { x.1 += 1 } else { x.2 += 1 },
                None => l.push((id, self.job as u32, !self.job as u32)),
            }
        }
    }
    fn release(&mut self) {
        if self.id == 0 {
            return;
        }
        if let Ok(mut l) = LEASES.lock() {
            if let Some(x) = l.iter_mut().find(|x| x.0 == self.id) {
                if self.job { x.1 = x.1.saturating_sub(1) } else { x.2 = x.2.saturating_sub(1) }
            }
            l.retain(|x| x.1 + x.2 > 0);
        }
        touch(self.id);
        self.id = 0;
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.release();
    }
}

fn touch(id: i64) {
    if let Ok(mut l) = LAST_USED.lock() {
        l.retain(|x| x.0 != id);
        l.push((id, Instant::now()));
    }
}

fn leased(id: i64, jobs_only: bool) -> bool {
    LEASES.lock().map(|l| l.iter().any(|x| x.0 == id && (x.1 > 0 || (!jobs_only && x.2 > 0)))).unwrap_or(false)
}

fn idle_for(id: i64) -> Duration {
    LAST_USED.lock().ok().and_then(|l| l.iter().find(|x| x.0 == id).map(|x| x.1.elapsed())).unwrap_or_default()
}

fn set_ready(id: i64, ready: bool) {
    if let Ok(mut r) = READY.lock() {
        r.retain(|x| *x != id);
        if ready {
            r.push(id);
        }
    }
}

pub fn is_ready(id: i64) -> bool {
    READY.lock().map(|r| r.contains(&id)).unwrap_or(false)
}

pub fn in_use(id: i64) -> bool {
    leased(id, true)
}

/// Max cards at the same time (setting `card_max`, 1-4, default 2).
pub fn card_max(st: &AppState) -> usize {
    st.db.kv_get("card_max").and_then(|v| v.parse::<usize>().ok()).unwrap_or(2).clamp(1, 4)
}

/// Per-card secrets: kv "cards" = {"<instance id>": {"token","cert","packs"}}.
fn meta_load(st: &AppState) -> serde_json::Map<String, Value> {
    st.db.kv_get("cards").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn meta_save(st: &AppState, m: &serde_json::Map<String, Value>) {
    st.db.kv_set("cards", &serde_json::to_string(m).unwrap_or_default()).ok();
}

fn meta_put(st: &AppState, id: i64, token: &str, cert: &str, packs: &str) {
    let mut m = meta_load(st);
    m.insert(id.to_string(), json!({ "token": token, "cert": cert, "packs": packs }));
    meta_save(st, &m);
}

pub struct Card {
    /// true when this call rented the card (the models were just installed), false when an existing card was reused
    pub fresh: bool,
    #[allow(dead_code)]
    pub id: i64,
    pub base: String,
    pub token: String,
    /// Talks TLS to the card and trusts only the certificate made for it (so the token never travels in clear text).
    pub client: reqwest::Client,
    _lease: Lease,
}

const HOST: &str = "aivs-card.local";
static STATE: std::sync::Mutex<(bool, String)> = std::sync::Mutex::new((false, String::new()));

fn set_state(ready: bool, msg: &str) {
    if let Ok(mut s) = STATE.lock() {
        *s = (ready, msg.to_string());
    }
}

/// (ready, progress text). `ready` is only true once ComfyUI on the card answers; the caller also checks that the card still exists.
pub fn state() -> (bool, String) {
    STATE.lock().map(|s| s.clone()).unwrap_or_default()
}

fn gen_cert() -> Result<(String, String), String> {
    let ck = rcgen::generate_simple_self_signed(vec![HOST.to_string()]).map_err(|e| e.to_string())?;
    Ok((ck.cert.pem(), ck.key_pair.serialize_pem()))
}

fn make_client(cert_pem: &str, ip: &str, port: &str) -> Result<reqwest::Client, String> {
    let cert = reqwest::Certificate::from_pem(cert_pem.as_bytes()).map_err(|e| e.to_string())?;
    let addr: std::net::SocketAddr = format!("{ip}:{port}").parse().map_err(|_| format!("bad card address {ip}:{port}"))?;
    reqwest::Client::builder().tls_built_in_root_certs(false).add_root_certificate(cert).resolve(HOST, addr).timeout(Duration::from_secs(120)).build().map_err(|e| e.to_string())
}

fn rand_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The packs (models.json ids) a workflow needs on the card.
pub fn packs_for(st: &AppState, pack_ids: &[String]) -> Vec<Value> {
    crate::catalog(st).into_iter().filter(|m| pack_ids.iter().any(|i| Some(i.as_str()) == m["id"].as_str())).collect()
}

/// aiohttp (already in ComfyUI's Python) in front of ComfyUI: only requests with our bearer token get through.
const PROXY: &str = r#"import os, ssl, hmac
from aiohttp import web, ClientSession
T = os.environ["AIVS_TOKEN"]
async def h(req):
    if not hmac.compare_digest(req.headers.get("Authorization", ""), "Bearer " + T):
        return web.Response(status=401)
    hd = {k: v for k, v in req.headers.items() if k.lower() not in ("host", "authorization", "content-length")}
    async with ClientSession() as s:
        async with s.request(req.method, "http://127.0.0.1:18188" + req.path_qs, data=await req.read(), headers=hd) as r:
            return web.Response(status=r.status, body=await r.read(), headers={"Content-Type": r.headers.get("Content-Type", "application/octet-stream")})
app = web.Application(client_max_size=300 * 1024 ** 2)
app.router.add_route("*", "/{p:.*}", h)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain("/workspace/aivs_cert.pem", "/workspace/aivs_key.pem")
web.run_app(app, port=8188, ssl_context=ctx, print=None)
"#;

/// Start script of the rented machine: downloads the models + nodes into the image's ComfyUI, then starts ComfyUI (local only) and the token proxy.
/// One download stream must reach this on the card (~400 Mbit/s; a single stream never fills a 1 Gbit/s line).
/// The slow cards seen in practice gave 1-3 MB/s.
const MIN_BYTES_PER_SEC: u64 = 50_000_000;

/// Machines that failed the speed test in the last 7 days (kv "slow_machines": {"machine id": unix seconds}).
fn slow_machines(st: &crate::AppState) -> Vec<i64> {
    let now = now_secs();
    let m: std::collections::HashMap<String, u64> = st.db.kv_get("slow_machines").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    m.into_iter().filter(|(_, t)| now.saturating_sub(*t) < 7 * 86400).filter_map(|(k, _)| k.parse().ok()).collect()
}

fn mark_slow(st: &crate::AppState, machine: i64) {
    if machine == 0 {
        return;
    }
    let now = now_secs();
    let mut m: std::collections::HashMap<String, u64> = st.db.kv_get("slow_machines").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    m.retain(|_, t| now.saturating_sub(*t) < 7 * 86400);
    m.insert(machine.to_string(), now);
    st.db.kv_set("slow_machines", &serde_json::to_string(&m).unwrap_or_default()).ok();
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn onstart(packs: &[Value], token: &str, cert: &str, key: &str) -> String {
    let mut script = provision::build_script(packs, true).replacen("C=/workspace/ComfyUI", "C=/opt/workspace-internal/ComfyUI; [ -d $C ] || C=/workspace/ComfyUI", 1);
    // Speed test before anything is downloaded: the link Vast advertises is often not what we really get.
    // Too slow -> marker line, the app closes this card and rents another one.
    let test_url = packs.iter().flat_map(|p| p["files"].as_array().cloned().unwrap_or_default()).filter(|f| f["url"].as_str().is_some_and(|u| u.starts_with("https://") && (u.ends_with(".safetensors") || u.ends_with(".gguf")))).max_by_key(|f| f["dir"].as_str().is_some_and(|d| d.contains("diffusion_models") || d.contains("unet"))).and_then(|f| f["url"].as_str().map(String::from));
    if let Some(u) = test_url {
        script = format!("S=$(curl -sL -r 0-199999999 --max-time 25 -o /dev/null -w '%{{speed_download}}' \"{u}\" 2>/dev/null); S=${{S%.*}}; S=${{S:-0}}; C=$(curl -s --max-time 15 -o /dev/null -w '%{{speed_download}}' 'https://speed.cloudflare.com/__down?bytes=100000000' 2>/dev/null); C=${{C%.*}}; echo AIVS_SPEED $((S/1000000)) MBps, neutral server $((${{C:-0}}/1000000)) MBps; if [ \"$S\" -lt {MIN_BYTES_PER_SEC} ]; then echo AIVS_SLOWLINK; exit 1; fi\n{script}");
    }
    script.push_str("echo AIVS_MODELS_DONE
mkdir -p /workspace
. /venv/main/bin/activate
cd $C
");
    script.push_str(&format!("cat > /workspace/aivs_cert.pem <<'CEOF'
{cert}CEOF
cat > /workspace/aivs_key.pem <<'CEOF'
{key}CEOF
chmod 600 /workspace/aivs_key.pem
"));
    script.push_str(&format!("cat > /workspace/aivs_proxy.py <<'PYEOF'
{PROXY}PYEOF
"));
    script.push_str("setsid nohup python main.py --listen 127.0.0.1 --port 18188 --disable-auto-launch > /workspace/comfy.log 2>&1 &
");
    script.push_str(&format!("AIVS_TOKEN={token} setsid nohup python /workspace/aivs_proxy.py > /workspace/proxy.log 2>&1 &
"));
    format!("#!/bin/bash
{}", provision::block(&script))
}

fn address(inst: &Value) -> Option<(String, String)> {
    let ip = inst["public_ipaddr"].as_str().filter(|s| !s.is_empty())?;
    let port = inst["ports"]["8188/tcp"].as_array()?.first()?["HostPort"].as_str()?;
    Some((ip.to_string(), port.to_string()))
}

async fn probe(c: &reqwest::Client, base: &str, token: &str, node: &str) -> bool {
    let Ok(r) = c.get(format!("{base}/object_info/{node}")).bearer_auth(token).timeout(Duration::from_secs(6)).send().await else { return false };
    r.status().is_success() && r.json::<Value>().await.map(|v| v.get(node).is_some()).unwrap_or(false)
}

/// Windows notification: the new card is up and all its models are downloaded.
pub fn notify_ready(app: &tauri::AppHandle, card: &Card) {
    use tauri_plugin_notification::NotificationExt;
    if !card.fresh {
        return;
    }
    let _ = app.notification().builder().title("AI Video Studio").body("The cloud card is ready and the models are downloaded. You can generate now.").show();
}

/// The model files of the packs that the card does not have (ComfyUI lists each models folder).
async fn missing_files(c: &reqwest::Client, base: &str, token: &str, packs: &[Value]) -> Vec<String> {
    let mut missing = vec![];
    let mut seen: std::collections::HashMap<String, Vec<String>> = Default::default();
    for p in packs {
        for f in p["files"].as_array().cloned().unwrap_or_default() {
            let (dir, url) = (f["dir"].as_str().unwrap_or(""), f["url"].as_str().unwrap_or(""));
            if dir.starts_with('@') || dir.is_empty() {
                continue;
            }
            let file = url.rsplit('/').next().unwrap_or("").to_string();
            if !seen.contains_key(dir) {
                let list = match c.get(format!("{base}/models/{dir}")).bearer_auth(token).timeout(Duration::from_secs(10)).send().await {
                    Ok(r) if r.status().is_success() => r.json::<Vec<String>>().await.unwrap_or_default(),
                    // cannot ask: do not block the card on a question ComfyUI did not answer
                    _ => vec![file.clone()],
                };
                seen.insert(dir.to_string(), list);
            }
            if !seen[dir].iter().any(|n| n == &file) {
                missing.push(format!("{dir}/{file}"));
            }
        }
    }
    missing
}

/// Returns a ready card: reuses an idle one that has the models, otherwise rents one (up to `card_max` at the same time).
/// `say` receives short progress texts.
pub async fn ensure(st: &AppState, gpu: &str, pack_ids: &[String], want: Want, cancel: &AtomicBool, say: &(dyn Fn(&str) + Sync)) -> Result<Card, String> {
    let key = secrets::get("vast_api_key").ok_or("Missing Vast API key (Settings).")?;
    if pack_ids.is_empty() {
        return Err("This mode has no model pack to install on a card.".into());
    }
    set_state(false, "Starting…");
    let say_outer = say;
    let say = |m: &str| {
        set_state(false, m);
        say_outer(m)
    };
    // what the user wants on the card: a picture-only card (just the picture model: cheaper and quicker to start),
    // or the mode's models (auto), Wan, FLUX or both at once
    let image_only = want.image && st.db.kv_get("image_card").as_deref() != Some("off");
    let mut ids: Vec<String> = pack_ids.to_vec();
    let extra: &[&str] = if image_only {
        &[]
    } else {
        match st.db.kv_get("card_models").as_deref() {
            Some("wan") => &[WAN_PACK],
            Some("flux") => &[FLUX_PACK],
            Some("auto") => &[],
            _ => &[WAN_PACK, FLUX_PACK],
        }
    };
    for e in extra {
        if !ids.iter().any(|i| i == e) {
            ids.push(e.to_string());
        }
    }
    let packs = packs_for(st, &ids);
    // a big model (Wan) needs the strong card; a card for small models only can be a cheaper one
    let big = packs.iter().any(|p| p["vram_gb"].as_f64().unwrap_or(0.0) > 16.0);
    let gpus: Vec<String> = if !gpu.is_empty() {
        vec![gpu.to_string()]
    } else if big {
        vec![st.db.kv_get("gpu_selection").and_then(|g| g.split(',').find(|x| !x.is_empty()).map(String::from)).unwrap_or_else(|| "RTX 4090".into())]
    } else {
        CHEAP_GPUS.iter().map(|s| s.to_string()).collect()
    };
    let want_packs = ids.join(",");
    let max = card_max(st);
    let mut lease = Lease::new(want.job);
    let mut token;
    let mut cert_pem;
    let mut key_pem;
    let mut offers: Vec<(i64, f64, String, i64)> = vec![];
    let mut cur_machine = 0i64;
    let mut tries = 0usize;
    // rents the next machine of the list; some machines cannot pull the image, then the next one is tried
    macro_rules! rent {
        () => {{
            if offers.is_empty() {
                say("Looking for a card with a fast link…");
                offers = st.vast.find_offers(&key, &gpus, DISK_GB, &slow_machines(st)).await?;
                if offers.is_empty() {
                    return Err(format!("No {} is available right now with enough disk and a 1 Gbit/s link. Try again in a few minutes or pick another card.", gpus.join(" / ")));
                }
            }
            tries += 1;
            if tries > 8 {
                return Err("Eight different machines were too slow or failed to start. Try again in a few minutes.".into());
            }
            let (offer, dph, gpu, machine) = offers.remove(0);
            cur_machine = machine;
            say(&format!("Renting a {gpu} (~{dph:.2} USD/hour)…"));
            // a new token and a new certificate for every new card
            token = rand_token();
            (cert_pem, key_pem) = gen_cert()?;
            let env = json!({ "-p 8188:8188": "1" });
            let new_id = st.vast.create_instance(&key, offer, json!({ "image": IMAGE, "env": env, "disk": DISK_GB, "label": LABEL, "runtype": "ssh", "onstart": onstart(&packs, &token, &cert_pem, &key_pem) })).await?;
            meta_put(st, new_id, &token, &cert_pem, &want_packs);
            lease.set(new_id);
            crate::log::info(&format!("card: created instance {new_id} ({gpu}, offer {offer}, {dph:.2} USD/h, packs {want_packs})"));
            new_id
        }};
    }
    // decide: reuse an idle card, or rent a new one if there is room (short critical section)
    let mut id;
    let reused;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let guard = LOCK.lock().await;
        let mut ours: Vec<Value> = st.vast.instances_raw(&key).await?.into_iter().filter(|i| i["label"] == LABEL).collect();
        let mut meta = meta_load(st);
        // a card made by version 0.7.x kept its secrets in single settings
        if meta.is_empty() && ours.len() == 1 {
            let (t, c, p) = (st.db.kv_get("card_token").unwrap_or_default(), st.db.kv_get("card_cert").unwrap_or_default(), st.db.kv_get("card_packs").unwrap_or_default());
            if !t.is_empty() && !c.is_empty() {
                meta_put(st, ours[0]["id"].as_i64().unwrap_or(0), &t, &c, &p);
                meta = meta_load(st);
            }
        }
        let alive: Vec<String> = ours.iter().filter_map(|i| i["id"].as_i64()).map(|n| n.to_string()).collect();
        let before = meta.len();
        meta.retain(|k, _| alive.contains(k));
        if meta.len() != before {
            meta_save(st, &meta);
        }
        let usable = |i: &Value| -> bool {
            let Some(n) = i["id"].as_i64() else { return false };
            let Some(m) = meta.get(&n.to_string()) else { return false };
            let have = m["packs"].as_str().unwrap_or("");
            !leased(n, true)
                && gpus.iter().any(|g| Some(g.as_str()) == i["gpu_name"].as_str())
                && want_packs.split(',').all(|p| have.split(',').any(|h| h == p))
                && !matches!(i["actual_status"].as_str(), Some("exited" | "stopped"))
        };
        let pick = if want.extra { None } else { ours.iter().filter(|i| usable(i)).min_by_key(|i| !is_ready(i["id"].as_i64().unwrap_or(0))).and_then(|i| i["id"].as_i64()) };
        if let Some(n) = pick {
            let m = &meta[&n.to_string()];
            token = m["token"].as_str().unwrap_or("").to_string();
            cert_pem = m["cert"].as_str().unwrap_or("").to_string();
            lease.set(n);
            id = n;
            reused = true;
            break;
        }
        // a card of ours whose secrets we do not have cannot be used: close it, it is billed otherwise
        for o in ours.clone() {
            let oid = o["id"].as_i64().unwrap_or(0);
            if oid != 0 && !meta.contains_key(&oid.to_string()) && !leased(oid, false) {
                crate::log::info(&format!("card: closing leftover {oid}"));
                let _ = st.vast.destroy_instance(&key, oid).await;
                ours.retain(|x| x["id"].as_i64() != Some(oid));
            }
        }
        if ours.len() >= max {
            if want.extra {
                return Err(format!("Already {} cards, which is the limit (setting card_max, 1-4). Close one or raise the limit first.", ours.len()));
            }
            // make room by closing the card that has been idle the longest (never one that is in use)
            let victim = ours.iter().filter_map(|i| i["id"].as_i64()).filter(|n| !leased(*n, false)).max_by_key(|n| idle_for(*n));
            match victim {
                Some(v) => {
                    crate::log::info(&format!("card: closing idle card {v} to make room for another kind"));
                    let _ = st.vast.destroy_instance(&key, v).await;
                    set_ready(v, false);
                }
                None => {
                    drop(guard);
                    say(&format!("All {max} cards are busy, waiting for one to be free…"));
                    tokio::time::sleep(Duration::from_secs(6)).await;
                    continue;
                }
            }
        }
        id = rent!();
        reused = false;
        break;
    }
    let mut fresh = !reused;
    // wait until ComfyUI answers with our node (it restarts after the models are downloaded)
    let mut t0 = Instant::now();
    let mut last_logs = Instant::now();
    let mut last_tail = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        if t0.elapsed() > Duration::from_secs(40 * 60) {
            let logs = st.vast.instance_logs(&key, id).await;
            let _ = st.vast.destroy_instance(&key, id).await;
            return Err(format!("The card was not ready after 40 minutes, so it was closed. Log end:\n{logs}"));
        }
        let list = st.vast.instances_raw(&key).await.unwrap_or_default();
        let Some(inst) = list.iter().find(|i| i["id"].as_i64() == Some(id)) else {
            set_ready(id, false);
            return Err("The card disappeared (Vast removed it).".into());
        };
        let status = inst["actual_status"].as_str().unwrap_or("?").to_string();
        if matches!(status.as_str(), "exited" | "stopped") {
            let _ = st.vast.destroy_instance(&key, id).await;
            set_ready(id, false);
            return Err("The card stopped by itself, so it was closed.".into());
        }
        if status == "running" {
            if let Some((ip, port)) = address(inst) {
                let base = format!("https://{HOST}:{port}");
                let client = make_client(&cert_pem, &ip, &port)?;
                if probe(&client, &base, &token, "AIVSSaveVideoBase64").await {
                    // ComfyUI answers, but are the model files really there? (a failed download used to go unnoticed)
                    let miss = missing_files(&client, &base, &token, &packs).await;
                    if !miss.is_empty() {
                        crate::log::info(&format!("card: {id} is up but lacks {}", miss.join(", ")));
                        let logs = st.vast.instance_logs(&key, id).await;
                        let _ = st.vast.destroy_instance(&key, id).await;
                        if fresh {
                            return Err(format!("The card started but these model files did not download: {}. The card was closed, so nothing keeps billing. Log end:\n{logs}", miss.join(", ")));
                        }
                        // an old card without the models we need: replace it
                        say("The card is missing models, renting a new one…");
                        id = rent!();
                        fresh = true;
                        t0 = Instant::now();
                        continue;
                    }
                    crate::log::info(&format!("card: {id} ready after {}s at {ip}:{port}", t0.elapsed().as_secs()));
                    set_state(true, "");
                    set_ready(id, true);
                    return Ok(Card { fresh, id, base, token, client, _lease: lease });
                }
                say(&format!("Card running, downloading the models… {}", crate::queue::fmt_secs(t0.elapsed().as_secs())));
                // the speed test runs in the first minute: look at the log often then, later every 30 seconds
                let every = match t0.elapsed().as_secs() { 0..=179 => 8, 180..=599 => 15, _ => 30 };
                if last_logs.elapsed() > Duration::from_secs(every) {
                    last_logs = Instant::now();
                    let l = st.vast.instance_logs(&key, id).await;
                    if l.contains("AIVS_SLOWLINK") {
                        let sp = l.lines().rev().find(|x| x.contains("AIVS_SPEED")).unwrap_or("").to_string();
                        crate::log::info(&format!("card: {id} download too slow ({sp}), machine {cur_machine} is remembered as slow, trying another"));
                        mark_slow(st, cur_machine);
                        let _ = st.vast.destroy_instance(&key, id).await;
                        id = rent!();
                        t0 = Instant::now();
                        continue;
                    }
                    if t0.elapsed().as_secs() > 600 && last_tail.elapsed() > Duration::from_secs(120) {
                        last_tail = Instant::now();
                        crate::log::info(&format!("card {id} log tail: {}", l.replace('\n', " | ")));
                    }
                }
            } else {
                say("Card running, opening the connection…");
            }
        } else {
            let msg = inst["status_msg"].as_str().unwrap_or("");
            if msg.contains("Error response from daemon") || t0.elapsed() > Duration::from_secs(12 * 60) {
                crate::log::info(&format!("card: instance {id} cannot start ({}), trying another machine", msg.chars().take(150).collect::<String>()));
                mark_slow(st, cur_machine);
                let _ = st.vast.destroy_instance(&key, id).await;
                id = rent!();
                t0 = Instant::now();
                continue;
            }
            say(&format!("Card {status}… {}", crate::queue::fmt_secs(t0.elapsed().as_secs())));
        }
        tokio::time::sleep(Duration::from_secs(8)).await;
    }
}

/// Checks every card of the app once a minute (the old check only ran while jobs were active):
/// - a card Vast stopped, or a ready card whose ComfyUI stopped answering for 3 minutes, is closed at once (a dead card still bills)
/// - a card that was never ready after 45 minutes is closed
/// - a ready card nobody uses for `auto_close_min` minutes is closed (the others keep working)
///
/// Each closing shows a Windows notification.
pub async fn watchdog(app: tauri::AppHandle) {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;
    let mut fails: std::collections::HashMap<i64, u32> = Default::default();
    let mut first_seen: std::collections::HashMap<i64, Instant> = Default::default();
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let st = app.state::<AppState>();
        let Some(key) = secrets::get("vast_api_key") else { continue };
        let Ok(list) = st.vast.instances_raw(&key).await else { continue };
        let ours: Vec<&Value> = list.iter().filter(|i| i["label"] == LABEL).collect();
        let meta = meta_load(&st);
        let limit: u64 = st.db.kv_get("auto_close_min").and_then(|v| v.parse().ok()).unwrap_or(10);
        let queued = st.db.next_queued().is_some();
        let ids: Vec<i64> = ours.iter().filter_map(|i| i["id"].as_i64()).collect();
        fails.retain(|k, _| ids.contains(k));
        first_seen.retain(|k, _| ids.contains(k));
        if let Ok(mut r) = READY.lock() {
            r.retain(|k| ids.contains(k));
        }
        for inst in ours {
            let Some(id) = inst["id"].as_i64() else { continue };
            let seen = *first_seen.entry(id).or_insert_with(Instant::now);
            let mut why = String::new();
            let status = inst["actual_status"].as_str().unwrap_or("");
            if matches!(status, "exited" | "stopped") {
                why = format!("Vast stopped the card ({status})");
            } else if status == "running" {
                if let (Some((ip, port)), Some(m)) = (address(inst), meta.get(&id.to_string())) {
                    let (token, cert) = (m["token"].as_str().unwrap_or(""), m["cert"].as_str().unwrap_or(""));
                    if let Ok(client) = make_client(cert, &ip, &port) {
                        let base = format!("https://{HOST}:{port}");
                        if probe(&client, &base, token, "AIVSSaveVideoBase64").await {
                            fails.remove(&id);
                            // answering after an app restart: the card is usable (its model files are checked when a job takes it)
                            if !leased(id, false) && !is_ready(id) {
                                set_ready(id, true);
                                touch(id);
                            }
                        } else if is_ready(id) {
                            let n = fails.entry(id).or_insert(0);
                            *n += 1;
                            if *n >= 3 {
                                why = "the card stopped answering for 3 minutes".into();
                            }
                        }
                    }
                }
            }
            if why.is_empty() && !is_ready(id) && !leased(id, false) && seen.elapsed() > Duration::from_secs(45 * 60) {
                why = "the card was never ready after 45 minutes".into();
            }
            if why.is_empty() && is_ready(id) && !leased(id, false) && !queued && limit > 0 && idle_for(id) > Duration::from_secs(limit * 60) {
                why = format!("nobody used the card for {limit} minutes");
            }
            if !why.is_empty() {
                crate::log::info(&format!("card watchdog: closing {id}: {why}"));
                let _ = st.vast.destroy_instance(&key, id).await;
                set_ready(id, false);
                let _ = app.notification().builder().title("AI Video Studio").body(format!("A cloud card was closed so it stops billing: {why}.")).show();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_blocks_second_job_but_not_a_warmup_wait() {
        let (mut warm, mut job) = (Lease::new(false), Lease::new(true));
        warm.set(9001);
        assert!(leased(9001, false) && !leased(9001, true), "a warm-up can be adopted by a job");
        job.set(9001);
        assert!(leased(9001, true), "a card in use by a job is taken");
        drop(job);
        assert!(!leased(9001, true));
        drop(warm);
        assert!(!leased(9001, false));
        assert!(idle_for(9001) < Duration::from_secs(5));
    }
}
