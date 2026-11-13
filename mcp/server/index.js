#!/usr/bin/env node
// AI Video Studio For Windows MCP server (stdio, no dependencies). Talks to the desktop app's local API.
// The app writes port+token to %APPDATA%\com.aivideostudio.app\api.json.
const fs = require("fs");
const path = require("path");
const readline = require("readline");

const INSTRUCTIONS = `Model choice: list_workflows returns for each mode its license (open = free on the user's PC, paid = billed per result by a provider and needs an API key; RunPod modes are billed per second of GPU time), good_for (what it does best) and est_usd_per_result (typical USD price of one result). When the user describes a task, recommend the best-fitting mode (e.g. text inside images -> a paid GPT Image mode, quick cheap photoreal images -> Nano Banana, long free clips -> open-source), state the price in USD, and ask before using a paid one.
AI Video Studio For Windows generates AI video, pictures, voice and tools (open-source models such as Wan 2.2, LTX 2.5, FLUX, MiniMax H3) on a RunPod Serverless GPU endpoint (cloud, billed per second only while a job runs) or on the user's own PC, from the user's own desktop app. The app must be running on the user's PC.
RunPod: there are no cards to rent and no models to install; the models already live on the endpoint's network volume. RunPod is either "asleep" (max workers 0: nothing can run, so nothing can bill) or "awake". generate_video wakes it up by itself, and the app puts it back to sleep after the queue has been empty for a few minutes (auto_sleep_min, default 5). The first video after sleep takes 1-3 minutes longer. Use runpod_status to show the state and the estimated USD spent this session (session_usd), runpod_wake to wake it in advance, and runpod_sleep (e.g. when the user says they are done) so that nothing can bill. (Ελληνικά: runpod_status = αν κοιμάται/ξυπνά και πόσα ξοδεύτηκαν, runpod_wake = ξύπνα το, runpod_sleep = βάλ' το για ύπνο.)
Workflow:
1. Call video_studio_status first. If it fails, tell the user to open the "AI Video Studio For Windows" app.
2. Each result costs real money (about 0.02-0.25 USD on RunPod, see est_usd_per_result). Before generate_video, tell the user the model, length and rough cost in USD and get a clear yes. Only then pass user_confirmed=true. Batches: one confirmation for the whole batch is enough if the user approved the count and cost. generate_video returns est_usd (typical price of that result in USD) and runs_on (runpod, local or an API provider): repeat est_usd to the user.
3. generate_video returns a job id at once; generation takes 1-15 minutes (waking RunPod is the slow part). Poll with get_job (wait_seconds up to 45) until status is done or failed. Do not start duplicate jobs while waiting. For several videos send them all together.
4. When done, file_path is the .mp4 on the user's disk (est_cost_usd is the measured cost on RunPod). Pass it to editing tools/skills. Report failures with the error text; make_report gives a diagnostic file.
Prompts: write English, cinematic, concrete (subject, action, camera, lighting, style). For several clips of one story keep the same style words and character description in each prompt. For image-to-video give image_path (a local file) and describe only the motion.
The user may name a model ("LTX", "Wan") in plain words. Pass it as workflow_id exactly as the user said; the app resolves it (when RunPod is the default place to run it picks the RunPod mode of that model). To change the app's permanent defaults ("use LTX from now on", "run on my PC", "sleep after 10 minutes") call set_defaults. A per-video workflow_id/provider overrides the defaults for that video only.
Models: ltxv-t2v / ltxv-i2v are fastest and cheapest; wan22-* are higher quality and slower. For pictures set kind="image" (FLUX or SDXL). Modes named fal-* and replicate-* run on hosted APIs (pay per result, no GPU, need the provider key); the others run on RunPod (provider="runpod", the default) or on the user's PC (provider="local"). The app's default place to run is shown by video_studio_status. Use list_workflows to see the options and their parameters.
More than video: list_workflows also has voice (kind tts: text to speech; vc: change a voice), picture tools (kind i2i: remove background, upscale, edit with words), smoother motion (v2v) and lip-sync (lipsync). Each entry lists its "inputs" (image/audio/video slots) and, for voices, its "languages": answer language questions from that list, never from memory. Chain steps with generate_video: set workflow_id to the tool and give "inputs" like {"image":"job:<id of the finished job>"} or {"audio":"C:\\\\path\\\\voice.wav"}. Typical chains: picture -> video (i2v), text -> voice (tts) -> lip-sync (video with a face + that voice), video -> v2v. Every step costs or takes time like a video, so confirm the whole plan once. Tools that exist only as open-source modes without a RunPod twin run on the user's PC. For voice cloning, voice change and lip-sync on a real person, first make sure the user confirms the voice/face is their own or they have permission; do not help impersonate someone or make people appear to say things they did not say. For "make me a video with Wan 2.2 from this picture" use workflow_id "wan22-i2v-lightning" with image_path; from text only use "wan22-t2v-lightning". When the user says they are finished, ask and call runpod_sleep so that nothing can bill (the app also does it by itself after a few idle minutes).`;

function apiInfo() {
  const base = process.env.AIVS_DATA_DIR || path.join(process.env.APPDATA || "", "com.aivideostudio.app");
  const f = path.join(base, "api.json");
  try {
    return JSON.parse(fs.readFileSync(f, "utf8"));
  } catch {
    throw new Error("Δεν βρέθηκε σύνδεση με το AI Video Studio For Windows (είτε το πρόγραμμα είναι κλειστό, είτε η σύνδεση με το Claude Desktop είναι απενεργοποιημένη στις Ρυθμίσεις του). Πες στον χρήστη να κάνει ΜΕ ΤΗ ΣΕΙΡΑ: 1) να ανοίξει το πρόγραμμα «AI Video Studio For Windows» και να το αφήσει ανοιχτό, 2) στο πρόγραμμα: Ρυθμίσεις (Settings) → κουτί «Claude Desktop connection» → να τσεκάρει το «Allow the Claude Desktop connection», 3) να ξαναζητήσει το ίδιο από εδώ. Αν έχει παλιά έκδοση του προγράμματος (πριν την 0.5.0), πρέπει πρώτα να εγκαταστήσει τη νέα.");
  }
}

async function call(method, route, body) {
  const info = apiInfo();
  let res;
  try {
    res = await fetch(`http://127.0.0.1:${info.port}${route}`, {
      method,
      headers: { Authorization: `Bearer ${info.token}`, "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new Error("Το AI Video Studio For Windows δεν απαντά (κλειστό ή με απενεργοποιημένη σύνδεση). Πες στον χρήστη να κάνει ΜΕ ΤΗ ΣΕΙΡΑ: 1) να ανοίξει το πρόγραμμα «AI Video Studio For Windows» και να το αφήσει ανοιχτό, 2) στο πρόγραμμα: Ρυθμίσεις (Settings) → κουτί «Claude Desktop connection» → να τσεκάρει το «Allow the Claude Desktop connection», 3) να ξαναζητήσει το ίδιο από εδώ. Αν έχει παλιά έκδοση του προγράμματος (πριν την 0.5.0), πρέπει πρώτα να εγκαταστήσει τη νέα.");
  }
  const text = await res.text();
  let data;
  try { data = JSON.parse(text); } catch { data = { error: text }; }
  if (res.status === 404) throw new Error("Το πρόγραμμα απέρριψε το αίτημα (παλιός κωδικός σύνδεσης). Πες στον χρήστη να κλείσει εντελώς και να ανοίξει ξανά το AI Video Studio For Windows, και μετά ξαναδοκίμασε.");
  if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  return data;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const DONE = new Set(["done", "failed", "cancelled"]);

const TOOLS = [
  {
    name: "video_studio_status",
    description: "Check that AI Video Studio For Windows is running and configured (API keys present, output folder, active jobs). Call this first.",
    inputSchema: { type: "object", properties: {} },
    run: () => call("GET", "/v1/status"),
  },
  {
    name: "list_workflows",
    description: "List available generation modes/models (id, kind t2v|i2v|flf2v, VRAM, typical seconds, adjustable parameters with defaults and limits).",
    inputSchema: { type: "object", properties: {} },
    run: () => call("GET", "/v1/workflows"),
  },
  {
    name: "runpod_status",
    description:
      "Show whether RunPod (the cloud GPU) is asleep, awake or working: state, workers_max (0 = asleep), jobs_waiting, session_usd (estimated USD spent by finished RunPod jobs since the app started), usd_per_hour_while_working and auto_sleep_min. Billing is per second only while a job runs; asleep means nothing can bill. Call it when the user asks what is running or what they are being charged. (Δείχνει αν το RunPod κοιμάται, είναι ξύπνιο ή δουλεύει και πόσα έχουν ξοδευτεί.)",
    inputSchema: { type: "object", properties: {} },
    run: () => call("GET", "/v1/runpod/status"),
  },
  {
    name: "runpod_wake",
    description:
      "Wake RunPod up now (sets the endpoint's maximum workers to 1) so the next video starts sooner. Not needed before generate_video, which wakes RunPod by itself. Costs nothing until a job runs. (Ξυπνά το RunPod.)",
    inputSchema: { type: "object", properties: {} },
    run: () => call("POST", "/v1/runpod/wake", {}),
  },
  {
    name: "runpod_sleep",
    description:
      "Put RunPod to sleep (maximum workers 0): nothing can run or be billed. Call it when the user says they are finished. A job that is still running may fail, so check runpod_status / list_jobs first. (Βάζει το RunPod για ύπνο, σταματά κάθε χρέωση.)",
    inputSchema: { type: "object", properties: {} },
    run: () => call("POST", "/v1/runpod/sleep", {}),
  },
  {
    name: "generate_video",
    description:
      "Queue ONE generation (costs real money). Ask the user for explicit confirmation including the approximate cost in USD first, then set user_confirmed=true. Returns a job id immediately plus est_usd (typical USD price of that result) and runs_on; RunPod is woken up automatically when it is asleep. Poll with get_job. Text-to-video by default; give image_path for image-to-video (and end_image_path for start->end frame).",
    inputSchema: {
      type: "object",
      required: ["prompt", "user_confirmed"],
      properties: {
        prompt: { type: "string", description: "English, cinematic, concrete description." },
        user_confirmed: { type: "boolean", description: "True only after the user explicitly agreed to the cost." },
        kind: { type: "string", enum: ["video", "image", "audio", "tts", "vc", "i2i", "v2v", "lipsync"], description: "image = still picture, audio = music, tts = speech, vc = voice change, i2i = picture tool, v2v = video tool, lipsync (default video)." },
        provider: { type: "string", enum: ["local", "runpod"], description: "Where to run open-source modes: the user's PC or RunPod (cloud GPU). Omit for the app default (RunPod)." },
        image_path: { type: "string", description: "Local path of a start image (png/jpg) for image-to-video." },
        end_image_path: { type: "string", description: "Local path of an end image (needs a flf2v workflow)." },
        workflow_id: { type: "string", description: "Model in the user's words (\"ltx\", \"wan\") or an id from list_workflows. Omit for the app's default." },
        width: { type: "integer" },
        height: { type: "integer" },
        length: { type: "integer", description: "Number of frames (LTX: 8n+1, e.g. 97 = ~4s at 24fps)." },
        fps: { type: "integer" },
        seed: { type: "integer" },
        negative: { type: "string" },
        project: { type: "string", description: "Project/folder name in the app (default \"Claude\")." },
        scene: { type: "integer", description: "Scene number; the file gets a sNN_ prefix (keeps story order)." },
        inputs: { type: "object", description: "Media inputs by slot key (see the workflow's inputs): value \"job:<job id>\" = result of an earlier job, or a local file path (png/jpg/webp, mp3/wav/flac/ogg/m4a, mp4/webm/mov)." },
        extra_params: { type: "object", description: "Other workflow parameters by key (see list_workflows)." },
      },
    },
    run: async (a) => {
      if (a.user_confirmed !== true) {
        throw new Error("Ρώτα πρώτα τον χρήστη (με κόστος) και ξανακάλεσε με user_confirmed=true.");
      }
      const params = { ...(a.extra_params || {}) };
      for (const k of ["width", "height", "length", "fps", "seed", "negative"]) if (a[k] !== undefined) params[k] = a[k];
      return call("POST", "/v1/jobs", {
        prompt: a.prompt, kind: a.kind, provider: a.provider, image_path: a.image_path, end_image_path: a.end_image_path, workflow_id: a.workflow_id,
        project: a.project, scene: a.scene, inputs: a.inputs, params,
      });
    },
  },
  {
    name: "set_defaults",
    description:
      "Change the app's defaults for all later videos (same as the settings in the app). model: free text like \"ltx\" or \"wan\". provider: \"runpod\" (cloud, default) or \"local\" (the user's PC). auto_sleep_min: minutes with an empty queue before RunPod goes to sleep (0 = never).",
    inputSchema: { type: "object", properties: { model: { type: "string" }, provider: { type: "string", enum: ["runpod", "local"], description: "Default place to run open-source modes." }, auto_sleep_min: { type: "integer", description: "Put RunPod to sleep after the queue has been empty for this many minutes (0 = never)." } } },
    run: (a) => call("POST", "/v1/settings", { model: a.model, provider: a.provider, auto_sleep_min: a.auto_sleep_min }),
  },
  {
    name: "get_job",
    description: "Status of a job. wait_seconds (max 45) waits until it finishes. status: queued|routing|generating|downloading|done|failed|cancelled. On done, file_path is the mp4.",
    inputSchema: { type: "object", required: ["job_id"], properties: { job_id: { type: "string" }, wait_seconds: { type: "integer" } } },
    run: async (a) => {
      const until = Date.now() + Math.min(Math.max(Number(a.wait_seconds) || 0, 0), 45) * 1000;
      let j = await call("GET", `/v1/jobs/${encodeURIComponent(a.job_id)}`);
      while (!DONE.has(j.status) && Date.now() < until) {
        await sleep(2000);
        j = await call("GET", `/v1/jobs/${encodeURIComponent(a.job_id)}`);
      }
      return j;
    },
  },
  {
    name: "list_jobs",
    description: "The 50 most recent jobs, newest first.",
    inputSchema: { type: "object", properties: {} },
    run: () => call("GET", "/v1/jobs"),
  },
  {
    name: "cancel_job",
    description: "Cancel a queued or running job.",
    inputSchema: { type: "object", required: ["job_id"], properties: { job_id: { type: "string" } } },
    run: (a) => call("POST", `/v1/jobs/${encodeURIComponent(a.job_id)}/cancel`, {}),
  },
  {
    name: "retry_job",
    description: "Run a failed/cancelled job again (costs money again; ask first).",
    inputSchema: { type: "object", required: ["job_id"], properties: { job_id: { type: "string" } } },
    run: (a) => call("POST", `/v1/jobs/${encodeURIComponent(a.job_id)}/retry`, {}),
  },
  {
    name: "make_report",
    description: "Write a diagnostic report file (settings without secrets, recent jobs, log tail) and return its path. Use when something fails repeatedly.",
    inputSchema: { type: "object", properties: {} },
    run: () => call("POST", "/v1/report", {}),
  },
];

function send(msg) {
  process.stdout.write(JSON.stringify(msg) + "\n");
}

async function handle(msg) {
  const { id, method, params } = msg;
  if (id === undefined) return; // notification
  try {
    if (method === "initialize") {
      return send({
        jsonrpc: "2.0", id,
        result: {
          protocolVersion: params?.protocolVersion || "2024-11-05",
          capabilities: { tools: {} },
          serverInfo: { name: "ai-video-studio", version: "0.4.0" },
          instructions: INSTRUCTIONS,
        },
      });
    }
    if (method === "ping") return send({ jsonrpc: "2.0", id, result: {} });
    if (method === "tools/list") {
      return send({ jsonrpc: "2.0", id, result: { tools: TOOLS.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })) } });
    }
    if (method === "tools/call") {
      const t = TOOLS.find((x) => x.name === params?.name);
      if (!t) return send({ jsonrpc: "2.0", id, error: { code: -32602, message: `Unknown tool ${params?.name}` } });
      try {
        const out = await t.run(params.arguments || {});
        return send({ jsonrpc: "2.0", id, result: { content: [{ type: "text", text: JSON.stringify(out, null, 2) }] } });
      } catch (e) {
        return send({ jsonrpc: "2.0", id, result: { isError: true, content: [{ type: "text", text: String(e.message || e) }] } });
      }
    }
    if (method === "resources/list") return send({ jsonrpc: "2.0", id, result: { resources: [] } });
    if (method === "prompts/list") return send({ jsonrpc: "2.0", id, result: { prompts: [] } });
    send({ jsonrpc: "2.0", id, error: { code: -32601, message: `Method not found: ${method}` } });
  } catch (e) {
    send({ jsonrpc: "2.0", id, error: { code: -32603, message: String(e.message || e) } });
  }
}

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  if (!line.trim()) return;
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  handle(msg);
});
