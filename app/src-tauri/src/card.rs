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

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct Card {
    /// true when this call rented the card (the models were just installed), false when an existing card was reused
    pub fresh: bool,
    pub id: i64,
    pub base: String,
    pub token: String,
    /// Talks TLS to the card and trusts only the certificate made for it (so the token never travels in clear text).
    pub client: reqwest::Client,
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
fn onstart(packs: &[Value], token: &str, cert: &str, key: &str) -> String {
    let mut script = provision::build_script(packs, true).replacen("C=/workspace/ComfyUI", "C=/opt/workspace-internal/ComfyUI; [ -d $C ] || C=/workspace/ComfyUI", 1);
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

/// Returns a ready card, creating one if none exists. `say` receives short progress texts.
pub async fn ensure(st: &AppState, gpu: &str, pack_ids: &[String], cancel: &AtomicBool, say: &(dyn Fn(&str) + Sync)) -> Result<Card, String> {
    let _guard = LOCK.lock().await;
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
    // what the user wants on the card: only the mode's models (auto), Wan, FLUX or both at once
    let mut ids: Vec<String> = pack_ids.to_vec();
    let extra: &[&str] = match st.db.kv_get("card_models").as_deref() {
        Some("wan") => &[WAN_PACK],
        Some("flux") => &[FLUX_PACK],
        Some("auto") => &[],
        _ => &[WAN_PACK, FLUX_PACK],
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
    let mut ours: Vec<Value> = st.vast.instances_raw(&key).await?.into_iter().filter(|i| i["label"] == LABEL).collect();
    let token_saved = st.db.kv_get("card_token").unwrap_or_default();
    let cert_saved = st.db.kv_get("card_cert").unwrap_or_default();
    let have_packs = st.db.kv_get("card_packs").unwrap_or_default();
    // reuse only a card that has the right GPU, the needed models and a token we know
    let reusable = ours.iter().position(|i| {
        gpus.iter().any(|g| Some(g.as_str()) == i["gpu_name"].as_str()) && !token_saved.is_empty() && !cert_saved.is_empty() && want_packs.split(',').all(|p| have_packs.split(',').any(|h| h == p)) && !matches!(i["actual_status"].as_str(), Some("exited" | "stopped"))
    });
    let mut id = 0i64;
    let mut reused = false;
    if let Some(ix) = reusable {
        reused = true;
        id = ours.remove(ix)["id"].as_i64().unwrap_or(0);
    }
    for o in ours {
        // an unusable leftover of ours (other GPU / models / no token): close it, it is billed otherwise
        if let Some(oid) = o["id"].as_i64() {
            crate::log::info(&format!("card: closing leftover {oid}"));
            let _ = st.vast.destroy_instance(&key, oid).await;
        }
    }
    let token = if id != 0 { token_saved } else { rand_token() };
    // a new token and a new certificate for every new card
    let (cert_pem, key_pem) = if id != 0 { (cert_saved, String::new()) } else { gen_cert()? };
    let mut offers: Vec<(i64, f64, String)> = vec![];
    let mut tries = 0usize;
    // rents the next machine of the list; some machines cannot pull the image, then the next one is tried
    macro_rules! rent {
        () => {{
            if offers.is_empty() {
                say("Looking for a card with a fast link…");
                offers = st.vast.find_offers(&key, &gpus, DISK_GB).await?;
                if offers.is_empty() {
                    return Err(format!("No {} is available right now with enough disk and a 1 Gbit/s link. Try again in a few minutes or pick another card.", gpus.join(" / ")));
                }
            }
            tries += 1;
            if tries > 4 {
                return Err("Four different machines failed to start. Try again in a few minutes.".into());
            }
            let (offer, dph, gpu) = offers.remove(0);
            say(&format!("Renting a {gpu} (~{dph:.2} USD/hour)…"));
            let env = json!({ "-p 8188:8188": "1" });
            let new_id = st.vast.create_instance(&key, offer, json!({ "image": IMAGE, "env": env, "disk": DISK_GB, "label": LABEL, "runtype": "ssh", "onstart": onstart(&packs, &token, &cert_pem, &key_pem) })).await?;
            st.db.kv_set("card_token", &token).ok();
            st.db.kv_set("card_cert", &cert_pem).ok();
            st.db.kv_set("card_packs", &want_packs).ok();
            crate::log::info(&format!("card: created instance {new_id} ({gpu}, offer {offer}, {dph:.2} USD/h)"));
            new_id
        }};
    }
    if id == 0 {
        id = rent!();
    }
    let mut fresh = !reused;
    // wait until ComfyUI answers with our node (it restarts after the models are downloaded)
    let mut t0 = Instant::now();
    let mut last_logs = Instant::now();
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
            return Err("The card disappeared (Vast removed it).".into());
        };
        let status = inst["actual_status"].as_str().unwrap_or("?").to_string();
        if matches!(status.as_str(), "exited" | "stopped") {
            let _ = st.vast.destroy_instance(&key, id).await;
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
                    return Ok(Card { fresh, id, base, token, client });
                }
                say(&format!("Card running, downloading the models… {}", crate::queue::fmt_secs(t0.elapsed().as_secs())));
                if last_logs.elapsed() > Duration::from_secs(120) {
                    last_logs = Instant::now();
                    let l = st.vast.instance_logs(&key, id).await;
                    crate::log::info(&format!("card {id} log tail: {}", l.replace('\n', " | ")));
                }
            } else {
                say("Card running, opening the connection…");
            }
        } else {
            let msg = inst["status_msg"].as_str().unwrap_or("");
            if msg.contains("Error response from daemon") || t0.elapsed() > Duration::from_secs(12 * 60) {
                crate::log::info(&format!("card: instance {id} cannot start ({}), trying another machine", msg.chars().take(150).collect::<String>()));
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
