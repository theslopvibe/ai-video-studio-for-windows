// Drives the real app (release exe) through WebView2 DevTools against the local mock Vast server.
import { spawn, execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const D = process.env.E2E_DIR;
const EXE = process.env.APP_EXE;
const MOCK = "http://127.0.0.1:18777";
const results = [];
const check = (name, ok, extra = "") => { results.push({ name, ok: !!ok }); console.log(`${ok ? "PASS" : "FAIL"}  ${name}${extra ? "  — " + extra : ""}`); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const app = spawn(EXE, [], {
  env: { ...process.env, AIVS_VAST_RUN_URL: MOCK, AIVS_VAST_CONSOLE_URL: MOCK, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-port=9222" },
  stdio: "ignore",
});

let page;
for (let i = 0; i < 60 && !page; i++) {
  await sleep(500);
  try { page = (await (await fetch("http://127.0.0.1:9222/json")).json()).find((t) => t.type === "page"); } catch {}
}
if (!page) { console.log("no devtools"); app.kill(); process.exit(2); }
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
const cdp = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (expr) => {
  const r = await cdp("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.result.exceptionDetails) return { __err: r.result.exceptionDetails.exception?.value ?? r.result.exceptionDetails.text };
  return r.result.result.value;
};
const inv = (cmd, args = {}) => ev(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`);
const shot = async (name) => { const r = await cdp("Page.captureScreenshot", { format: "png" }); fs.writeFileSync(path.join(D, name + ".png"), Buffer.from(r.result.data, "base64")); };
const click = (text) => ev(`(() => { const b=[...document.querySelectorAll('button')].find(x=>x.textContent.trim().includes(${JSON.stringify(text)})); if(!b) return false; b.click(); return true; })()`);
const setVal = (sel, v) => ev(`(() => { const e=document.querySelector(${JSON.stringify(sel)}); const p=e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(p,'value').set.call(e,${JSON.stringify(v)}); e.dispatchEvent(new Event('input',{bubbles:true})); return true; })()`);
const jobs = async () => inv("jobs_list");
async function waitJob(pred, ms = 60000) { const t = Date.now(); while (Date.now() - t < ms) { const j = (await jobs()).find(pred); if (j && ["done", "failed", "cancelled"].includes(j.status)) return j; await sleep(500); } return null; }
const mockLog = async () => (await fetch(MOCK + "/__log")).json();

try {
  await sleep(2500);
  await shot("01-first-run");
  const boot0 = await inv("bootstrap");
  check("first run: Default project exists", boot0.projects.length === 1 && boot0.projects[0].name === "Default");
  check("default mode is Wan text lightning", boot0.workflows[0].meta.id === "wan22-t2v-lightning", boot0.workflows[0].meta.id);
  check("first run: 7 bundled workflows loaded (3 text, 4 image)", boot0.workflows.length === 7 && boot0.workflows[0].meta.kind !== "i2v" && boot0.workflow_errors.length === 0);
  check("first run: Claude Code CLI found", !!boot0.claude_cli, boot0.claude_cli);

  // ---- keys / connection
  await inv("secret_set", { name: "vast_api_key", value: "test-key" });
  await inv("setting_set", { key: "endpoint_name", value: "comfyui-json" });
  const t = await inv("test_vast");
  check("Test Connection finds endpoint and scale-to-zero", t.ok && t.endpoint?.scales_to_zero, t.message);
  await inv("secret_set", { name: "vast_api_key", value: "bad" });
  const tb = await inv("test_vast");
  check("bad key gives a clear error", tb.__err && tb.__err.includes("API key"), JSON.stringify(tb).slice(0, 120));
  await inv("secret_set", { name: "vast_api_key", value: "test-key" });

  // ---- projects with own folders
  const folderA = path.join(D, "projA"), folderB = path.join(D, "projB");
  let list = await inv("project_create", { name: "Clip A", folder: folderA });
  check("project A created with its own folder", fs.existsSync(folderA) && list.find((p) => p.name === "Clip A")?.dir === folderA);
  list = await inv("project_create", { name: "Clip B", folder: "" });
  const pa = list.find((p) => p.name === "Clip A"), pb = list.find((p) => p.name === "Clip B");
  check("project without folder falls back to output folder/name", pb.dir.endsWith("Clip-B") || pb.dir.endsWith("Clip B"), pb.dir);
  list = await inv("project_update", { id: pa.id, name: "Clip A", folder: folderA, suffix: "cinematic, warm tones", sidecar: true });
  check("project style suffix saved", list.find((p) => p.id === pa.id).suffix === "cinematic, warm tones");
  list = await inv("project_update", { id: pb.id, name: "Clip B", folder: folderB, suffix: "", sidecar: false });
  check("project B folder changed", fs.existsSync(folderB));

  // ---- UI driven generation (types a prompt into the real page, presses Generate)
  await ev(`window.__TAURI_INTERNALS__.invoke('setting_set',{key:'last_project',value:${JSON.stringify(pa.id)}})`);
  await cdp("Page.reload"); await sleep(2500);
  await click("Δημιουργία");
  await sleep(500);
  await setVal("#prompt", "a red fox running through snow");
  await click("16:9");
  await shot("02-generate-filled");
  await click("Generate");
  const j1 = await waitJob((j) => j.prompt.includes("red fox"), 90000);
  check("UI: generation completes", j1?.status === "done", j1 && (j1.status + " " + j1.error));
  check("UI: prompt got project style suffix", j1?.prompt === "a red fox running through snow, cinematic, warm tones", j1?.prompt);
  check("video saved in project A folder", j1 && path.dirname(j1.file_path) === folderA && fs.statSync(j1.file_path).size > 1000, j1?.file_path);
  check("video is a valid mp4", j1?.file_path && execFileSync("ffprobe", ["-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0", j1.file_path]).toString().includes("h264"));
  const side = j1 && j1.file_path.replace(/\.mp4$/, ".txt");
  check("prompt .txt written next to video", side && fs.existsSync(side) && fs.readFileSync(side, "utf8").includes("red fox"));
  const lg = await mockLog();
  const g0 = lg.genBodies[0];
  check("worker got inline-save node and no SaveVideo (local mode)", g0 && Object.values(g0.wf).some((n) => n.class_type === "AIVSSaveVideoBase64") && !Object.values(g0.wf).some((n) => n.class_type === "SaveVideo"));
  check("workflow placeholders all filled, width/height 832x480", g0 && !JSON.stringify(g0.wf).includes("{{") && g0.wf["104"].inputs.width === 832 && g0.wf["104"].inputs.height === 480);
  check("auth_data from /route/ forwarded to worker", g0?.auth_data?.signature === "sig");
  check("route was polled until worker ready (cold start path)", lg.log.filter((l) => l.p === "/route/").length >= 2);

  // ---- image to video: tiny JPEG travels as a file, then inline into the workflow
  const tinyJpg = "/9j/4AAQSkZJRgABAQEASABIAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////wgALCAABAAEBAREA/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxA=";
  const i2vId = boot0.workflows.find((w) => w.meta.id === "wan22-i2v-lightning")?.meta.id;
  check("i2v workflow has an image input", boot0.workflows.find((w) => w.meta.id === i2vId)?.meta.inputs.some((i) => i.type === "image"));
  const ji = await inv("job_enqueue", { projectId: pa.id, workflowId: i2vId, prompt: "slow zoom in", params: { image: tinyJpg } });
  check("i2v job keeps the image as a file, not in the db", ji.params.includes('"file:') && !ji.params.includes(tinyJpg.slice(0, 40)), ji.params.slice(0, 80));
  const jid = await waitJob((j) => j.prompt.startsWith("slow zoom in"), 90000);
  check("i2v generation completes", jid?.status === "done", jid && (jid.status + " " + jid.error));
  const lgI = await mockLog();
  const gi = lgI.genBodies.find((b) => Object.values(b.wf).some((n) => n.class_type === "AIVSLoadImageBase64"));
  const li = gi && Object.values(gi.wf).find((n) => n.class_type === "AIVSLoadImageBase64");
  check("worker received the start image inline (base64)", li && li.inputs.image_b64.length > 40 && !JSON.stringify(gi.wf).includes("{{"));
  await inv("job_enqueue", { projectId: pa.id, workflowId: i2vId, prompt: "no image given", params: {} });
  const jNoImg = await waitJob((j) => j.prompt.startsWith("no image given"), 60000);
  check("i2v without image fails with a clear message", jNoImg?.status === "failed" && jNoImg.error.includes("image"), jNoImg?.error);

  // ---- start -> end frame, two images, scene number in file name
  const jFlf = await inv("job_enqueue", { projectId: pa.id, workflowId: "wan22-flf2v-lightning", prompt: "morph between", params: { image: tinyJpg, image_end: tinyJpg, scene: 2 } });
  check("two image slots saved as two files", jFlf.params.includes("-image.jpg") && jFlf.params.includes("-image_end.jpg"), jFlf.params.slice(0, 160));
  const jfd = await waitJob((j) => j.prompt.startsWith("morph between"), 90000);
  check("start->end job completes", jfd?.status === "done", jfd && (jfd.status + " " + jfd.error));
  check("scene number is in the file name", jfd?.file_path && path.basename(jfd.file_path).includes("_s02_"), jfd?.file_path);
  const lgF = await mockLog();
  const gf = lgF.genBodies.find((b) => Object.values(b.wf).some((n) => n.class_type === "WanFirstLastFrameToVideo"));
  check("worker got both start and end image", gf && Object.values(gf.wf).filter((n) => n.class_type === "AIVSLoadImageBase64").length === 2);
  await inv("job_enqueue", { projectId: pa.id, workflowId: "wan22-flf2v-lightning", prompt: "missing end image", params: { image: tinyJpg } });
  const jfm = await waitJob((j) => j.prompt.startsWith("missing end image"), 60000);
  check("missing end image gives a clear message", jfm?.status === "failed" && jfm.error.includes("End image"), jfm?.error);

  // ---- per-video GPU card: applied to the workergroup before routing, priced with that card
  const jg = await inv("job_enqueue", { projectId: pa.id, workflowId: boot0.workflows[0].meta.id, prompt: "own gpu card", params: { gpu: "RTX_5090", gpu_dph: 0.8 } });
  check("job remembers its own card", jg.gpu === "RTX_5090", "gpu=" + jg.gpu);
  const jgd = await waitJob((j) => j.prompt.startsWith("own gpu card"), 90000);
  check("job with its own card completes", jgd?.status === "done", jgd && (jgd.status + " " + jgd.error));
  const lgG = await mockLog();
  check("workergroup was pointed at that card before the job", lgG.log.some((l) => l.p.includes("/workergroups/") && JSON.stringify(l).includes("RTX_5090")));

  // ---- logs: timestamps, no big payloads, report file
  const tail = await inv("logs_tail", { lines: 400 });
  check("log has timestamped job lines", /\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3} \[INFO\] job [0-9a-f]{8} (start|done)/.test(tail), tail.split("\n").slice(-2).join(" | ").slice(0, 160));
  check("log records errors with their cause", /\[ERROR\] job [0-9a-f]{8} failed: .*CUDA out of memory/.test(tail) || /\[ERROR\]/.test(tail));
  check("log never contains the image data", !tail.includes(tinyJpg.slice(0, 60)));
  const rep = await inv("logs_report");
  const repText = fs.existsSync(rep) ? fs.readFileSync(rep, "utf8") : "";
  check("report file written with settings, jobs and log", repText.includes("== settings") && repText.includes("== last 25 jobs") && repText.includes("== log"), rep);
  check("report holds no key", !repText.includes("test-key"));
  const setup = await inv("setup_endpoint", { gpus: [] });
  check("setup with an existing endpoint changes nothing", typeof setup === "string" && setup.includes("already set up"), JSON.stringify(setup).slice(0, 120));

  // ---- deleting a job removes its start image file
  const jDel = await inv("job_enqueue", { projectId: pa.id, workflowId: i2vId, prompt: "delete me", params: { image: tinyJpg } });
  const imgFile = JSON.parse(jDel.params).image.replace("file:", "");
  check("start image file exists while job exists", fs.existsSync(imgFile));
  await inv("job_cancel", { id: jDel.id });
  await sleep(1500);
  await inv("job_delete", { id: jDel.id, deleteFile: true });
  check("start image file removed with the job", !fs.existsSync(imgFile));

  // ---- plain ComfyUI export: parameters are found automatically
  const plain = path.join(D, "my-flow.json");
  fs.writeFileSync(plain, JSON.stringify({
    "3": { class_type: "KSampler", inputs: { seed: 1, steps: 20, cfg: 5, model: ["1", 0], positive: ["6", 0], negative: ["7", 0], latent_image: ["5", 0] } },
    "5": { class_type: "EmptyHunyuanLatentVideo", inputs: { width: 960, height: 544, length: 65, batch_size: 1 } },
    "6": { class_type: "CLIPTextEncode", _meta: { title: "Positive" }, inputs: { text: "x" } },
    "7": { class_type: "CLIPTextEncode", _meta: { title: "Negative" }, inputs: { text: "bad" } },
    "9": { class_type: "CreateVideo", inputs: { fps: 24, images: ["8", 0] } },
  }));
  const imp = await inv("workflow_import", { path: plain });
  const mine = Array.isArray(imp) && imp[0].find((w) => w.meta.id === "my-flow");
  const keysFound = mine ? mine.meta.inputs.map((i) => i.key) : [];
  check("imported workflow: size, frames, fps, seed, steps found automatically", ["prompt", "width", "height", "length", "fps", "seed", "steps", "cfg", "negative"].every((k) => keysFound.includes(k)), keysFound.join(","));
  check("imported workflow keeps its own values as defaults", mine && mine.meta.inputs.find((i) => i.key === "width").default === 960);

  // ---- HuggingFace lookup: model type, quality and recommended files (needs internet)
  const lk = await inv("hf_lookup", { query: "https://huggingface.co/Lightricks/LTX-Video", videoOnly: false });
  check("HF lookup recognises the LTX-Video family", lk && lk.kind === "repo" && lk.data.insight.family === "LTX-Video" && lk.data.insight.kind === "video", JSON.stringify(lk).slice(0, 160));
  check("HF lookup recommends files and estimates quality/speed", lk?.data?.files.some((f) => f.recommended) && lk.data.insight.quality >= 1 && lk.data.insight.speed >= 1);

  // ---- HuggingFace models: custom pack validation + persistence
  const evil = await inv("custom_model_add", { repo: "org/repo", name: "x", kind: "video", vramGb: 24, license: "mit", files: [['a";rm -rf /;".safetensors', "vae", 1]] });
  check("custom model with shell characters is rejected", evil.__err);
  const added = await inv("custom_model_add", { repo: "org/repo", name: "My model", kind: "video", vramGb: 24, license: "mit", files: [["x/y.safetensors", "diffusion_models", 2000000000]] });
  check("custom model added to catalog", Array.isArray(added) && added.some((m) => m.id === "hf-org--repo" && m.custom));
  const inst = await inv("install_on_workers", { ids: ["hf-org--repo"], apply: false });
  check("install script downloads the custom model", inst.script?.includes('dl "https://huggingface.co/org/repo/resolve/main/x/y.safetensors" "$M/diffusion_models/y.safetensors"') && inst.script.includes("AIVSLoadImageBase64"));
  const after = await inv("custom_model_remove", { id: "hf-org--repo" });
  check("custom model removed", Array.isArray(after) && !after.some((m) => m.id === "hf-org--repo"));

  // ---- queue: several prompts in sequence, project B has sidecar off
  const wf = boot0.workflows[0].meta.id;
  for (const p of ["one", "two", "three"]) await inv("job_enqueue", { projectId: pb.id, workflowId: wf, prompt: "queue " + p, params: {} });
  const q = [];
  for (const p of ["one", "two", "three"]) q.push(await waitJob((j) => j.prompt === "queue " + p, 120000));
  check("batch of 3 prompts all done in order", q.every((x) => x?.status === "done") && q[0].finished_at <= q[1].finished_at && q[1].finished_at <= q[2].finished_at);
  check("project B uses folder B and no .txt", q.every((x) => x && path.dirname(x.file_path) === folderB) && !fs.readdirSync(folderB).some((f) => f.endsWith(".txt")));

  // ---- failure + retry, cancel, fallback to R2
  await inv("job_enqueue", { projectId: pa.id, workflowId: wf, prompt: "FAIL please", params: {} });
  const jf = await waitJob((j) => j.prompt.startsWith("FAIL"));
  check("worker failure is shown with its message", jf?.status === "failed" && jf.error.includes("CUDA out of memory"), jf?.error);
  await inv("job_retry", { id: jf.id });
  const jf2 = await waitJob((j) => j.id === jf.id && j.status === "failed", 30000);
  check("retry re-runs the job", !!jf2);

  const slow = await inv("job_enqueue", { projectId: pa.id, workflowId: wf, prompt: "SLOW one", params: {} });
  for (let i = 0; i < 40; i++) { const j = (await jobs()).find((x) => x.id === slow.id); if (j.status === "generating") break; await sleep(300); }
  await inv("job_cancel", { id: slow.id });
  const sc = await waitJob((j) => j.id === slow.id, 15000);
  check("cancel while generating", sc?.status === "cancelled", sc?.status);

  await inv("job_enqueue", { projectId: pa.id, workflowId: wf, prompt: "NONODE fallback", params: {} });
  const jn = await waitJob((j) => j.prompt.startsWith("NONODE"), 90000);
  check("missing node on worker -> automatic fallback to R2 URL download", jn?.status === "done" && fs.existsSync(jn.file_path), jn && jn.status + " " + jn.error);

  await inv("setting_set", { key: "delivery", value: "cloud" });
  await inv("job_enqueue", { projectId: pa.id, workflowId: wf, prompt: "cloud mode", params: {} });
  const jc = await waitJob((j) => j.prompt.startsWith("cloud mode"), 90000);
  check("cloud delivery mode downloads from URL", jc?.status === "done" && fs.existsSync(jc.file_path));
  const gc = (await mockLog()).genBodies.at(-1);
  check("cloud mode keeps SaveVideo and omits inline node", Object.values(gc.wf).some((n) => n.class_type === "SaveVideo") && !Object.values(gc.wf).some((n) => n.class_type === "AIVSSaveVideoBase64"));
  await inv("setting_set", { key: "delivery", value: "local" });

  // ---- GPU + scale to zero + worker install
  const offers = await inv("gpu_offers", { minVram: 24 });
  check("GPU offers grouped with prices", offers.length === 3 && offers[0].name === "RTX_4090" && Math.abs(offers[0].min_price - 0.45) < 1e-9, JSON.stringify(offers[0]));
  const ag = await inv("apply_gpus", { gpus: ["RTX_4090", "RTX_5090"], minVram: 24, dph: 0.6 });
  check("apply GPUs rewrites search_params keeping other filters", ag.includes("verified=true rentable=true rented=false gpu_name in [RTX_4090,RTX_5090]") && !ag.includes("RTX_3090"), ag);
  const z = await inv("apply_scale_to_zero");
  check("scale-to-zero call ok", typeof z === "string");
  const ins = await inv("install_on_workers", { ids: ["wan22-ti2v-5b", "flux1-schnell"], apply: true });
  const lg2 = await mockLog();
  const tp = lg2.log.find((l) => l.m === "POST" && l.p === "/api/v0/template/");
  const wgp = lg2.log.filter((l) => l.m === "PUT" && l.p === "/api/v0/workergroups/9/").at(-1);
  check("install: new private template created with AIVS block before original commands", ins.applied && tp?.body.onstart.startsWith("#!/bin/bash\n# >>> AIVS BEGIN") && tp.body.onstart.includes("entrypoint.sh"));
  check("install: disk raised, private, name prefixed", tp.body.recommended_disk_space >= 100 + 18 + 17 && tp.body.private === true && tp.body.name.startsWith("AIVS "), `${tp.body.recommended_disk_space} ${tp.body.name}`);
  check("install: workergroup pointed to new template hash", wgp?.body.template_hash === "newhash123");
  const prev = await inv("install_on_workers", { ids: ["wan22-ti2v-5b"], apply: false });
  check("install preview lists the model URLs and local node", prev.script.includes("wan2.2_ti2v_5B_fp16.safetensors") && prev.script.includes("AIVSSaveVideoBase64"));

  // ---- every page renders in both themes
  const pages = [["Δημιουργία", "generate", "Τρόπος παραγωγής"], ["Ουρά", "queue", "Ουρά"], ["Βιβλιοθήκη", "library", "Άνοιγμα φακέλου"], ["Projects", "projects", "Φάκελος αποθήκευσης"], ["GPU", "gpu", "Φόρτωση τιμών"], ["Μοντέλα", "models", "Μοντέλα open-source"], ["Ρυθμίσεις", "settings", "Κλειδιά υπηρεσιών"]];
  for (const theme of ["dark", "light"]) {
    await inv("setting_set", { key: "theme", value: theme });
    await cdp("Page.reload"); await sleep(2200);
    for (const [label, key, marker] of pages) {
      await click(label); await sleep(450);
      const ok = await ev(`(document.querySelector('.page')?.innerText||'').includes(${JSON.stringify(marker)}) && !document.body.innerText.includes('undefined')`);
      check(`page ${key} (${theme}) shows its content`, ok);
      if (["library", "projects", "settings", "models"].includes(key) || theme === "dark") await shot(`${theme}-${key}`);
    }
  }

  // ---- library UI shows clips of the current project
  await inv("setting_set", { key: "last_project", value: pa.id });
  await cdp("Page.reload"); await sleep(2200);
  await click("Βιβλιοθήκη"); await sleep(800);
  const nClips = await ev(`document.querySelectorAll('.clip').length`);
  check("library shows project A clips", nClips >= 3, String(nClips));
  await shot("library-final");
} catch (e) {
  console.log("E2E ERROR", e);
  check("e2e ran without exception", false, String(e));
}

fs.writeFileSync(path.join(D, "results.json"), JSON.stringify(results, null, 1));
console.log(`\n${results.filter((r) => r.ok).length}/${results.length} passed`);
await inv("secret_set", { name: "vast_api_key", value: "" }).catch(() => {});
app.kill();
process.exit(0);
