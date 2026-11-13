---
tags: [ai-video-studio, research, providers, modal, beam]
---
# Research: Beam and Modal as Vast replacements (large cards)

Read 2026-11-13. Every number was read from the cited page on that date. "unverified" = not confirmed from a primary source. No money spent, no accounts created. Extends [[Research - Vast alternatives and face swap]] (RunPod pick, Modal runner-up, Beam cheapest). Context: [[Vast protocol]], [[Next steps]].

## TL;DR
- **Modal is the better fit for large cards.** All of L40S, A100 80GB, H100, H200, B200 are true serverless per-second GPUs, there is an official ComfyUI-as-API example (H100), and volumes are documented. No 4090.
- **Beam only does serverless on RTX 4090 ($0.69/h) and RTX 5090 ($1.09/h, committed-spend only).** H100/H200/A100/L40S/B200 are "on-demand machines" at a flat hourly rate, a different (Vast-like) model. So Beam is a 4090 play, not a large-card play.
- **Cost per 5 s Wan 2.2 14B video (assumptions in section 6):** Modal H100 ~$0.13-0.15, Beam 4090 ~$0.03-0.06 (using the app's measured 76 s), RunPod H100 ~$0.20 (single hourly price from the old note).
- **Biggest unknown for all three:** cold start with 30-50 GB of weights. Only vendor examples exist (Modal Flux/H100 ~1 min; Beam Flux/4090 ~90 s). No Wan 2.2 14B number found.
- **Verdict:** validate **Modal H100 first** (cost $0 inside the $30 Starter credits), RunPod stays fallback, Beam is an optional cheap-4090 candidate for later (section 7).

## 1. Facts per provider

### Modal ([pricing](https://modal.com/pricing))
| Item | Value |
|---|---|
| GPUs, per second | B300 $0.001972, B200 $0.001736, H200 $0.001261, H100 $0.001097, RTX PRO 6000 $0.000842, A100 80GB $0.000694, A100 40GB $0.000583, L40S $0.000542, A10 $0.000306, L4 $0.000222, T4 $0.000164 |
| CPU / RAM (billed separately) | $0.0000131 per core-s (1 core = 2 vCPU), $0.00000222 per GiB-s |
| Idle / scale to zero | $0 at zero. Idle containers stay up `scaledown_window` (default 60 s, range 2 s to 20 min) and are billed; `min_containers`, `buffer_containers` cost money [cold-start doc](https://modal.com/docs/guide/cold-start) |
| Limits by plan | Starter: $0/mo + usage, **$30 free credits/mo**, 100 containers, **10 concurrent GPUs**, 3 seats, 1 TiB free egress, 1-day logs. Team: $250/mo, $100 credits, 50 GPUs, 5,000 containers. Egress $0.04/GiB beyond free |
| Multipliers | Region pinning 1.15-1.75x; non-preemptible 3x (but `nonpreemptible` is **not supported for GPU Functions**, so GPU jobs can be preempted and restarted on the same input) [preemption](https://modal.com/docs/guide/preemption) |
| GPU facts | up to 8 GPUs per container; fallback lists (`gpu=["H100","A100-80GB"]`); `gpu="H100"` may silently run on H200 at the same price, `"H100!"` opts out [gpu doc](https://modal.com/docs/guide/gpu) |
| Volumes | $0.09/GiB-month, page says "including 1 TiB per month free" (I read it as a free allowance on volume storage; confirm in billing before relying). Billed on a daily snapshot of total size; deleted data may bill up to 4 days [volumes](https://modal.com/docs/guide/volumes) |

### Beam ([pricing](https://www.beam.cloud/pricing), [serverless-gpu](https://www.beam.cloud/serverless-gpu))
| Item | Value |
|---|---|
| Serverless GPUs | **RTX 4090 24 GB $0.69/h ($0.000192/s), any plan. RTX 5090 32 GB $1.09/h ($0.000303/s), needs a committed-spend agreement.** Billed per ms while code runs; no charge for cold start (container spin-up). Warm time (default 180 s on endpoints) is billed |
| CPU / RAM with serverless GPU | pricing page: $0.18972/h per vCPU, $0.0198/h per GiB. serverless-gpu page: $0.000105 per core-s, $0.0000055 per GiB-s (the two pages differ in unit; either way adds cost) |
| On-demand machines (flat hourly, incl. vCPU/RAM/NVMe) | B200 from $4.11, H200 $2.09, H100 PCIe $1.83, A100 80GB SXM $1.36, RTX PRO 6000 $1.09, L40S $0.76, 5090 $0.72, A6000 $0.54, 4090 $0.44 per hour |
| Large cards as serverless? | **Not listed.** Docs: "GPU model names supported by the SDK are not a promise that every model is currently available serverless" [gpu doc](https://docs.beam.cloud/v2/environment/gpu.md). Beam's own blog example uses an H100 at $1.74/h for a ComfyUI endpoint [blog](https://www.beam.cloud/blog/comfyui-api), which conflicts with the pricing page (H100 $1.83 on-demand). Whether `gpu="H100"` on an endpoint scales to zero and bills per ms: **unverified, must be tested** |
| Reserved / pools | Reserved nodes bill hourly from reservation even idle; stopping an app does not release the machine; `beam machine release` / `beam pool delete` needed [pools](https://docs.beam.cloud/v2/scaling/pools.md). An always-on H100 at $1.83/h = ~$1,318/mo |
| Plans | Developer $20/mo incl. $20 credit, **5 GPUs, 30 vCPU concurrency**; Team $89/mo, 50 GPUs; "purchased credits don't expire"; no free tier listed |
| Containers | `min_containers`, `max_containers`, `tasks_per_container`, `QueueDepthAutoscaler` [scaling](https://docs.beam.cloud/v2/scaling/concurrency.md). Warm containers count toward concurrency |
| Storage | **Not billed yet; planned $0.0001/GB-h = $0.072/GB-month (150 GB ~ $10.80/mo), no free allowance** [volume doc](https://docs.beam.cloud/v2/data/volume.md) |

## 2. Running our ComfyUI API-format workflow

### Modal
- Official example "Run Flux on ComfyUI as an API": `Inference` class on `gpu="H100"`, 10 min timeout, Volume `hf-hub-cache` mounted at `/cache`; "Inference takes about one minute to cold start" then 1-2 s per image [example](https://modal.com/docs/examples/comfyapp). Outputs there are PNG only; video not shown.
- Search snippet of the same example mentions a baked `workflow_api.json`, edited prompt fields, a custom node to support memory snapshots (could not read the full code: partly unverified). Modal blog (Feb 2025) reports ComfyUI cold start 10-15 s down to under 3 s with snapshots [search listing](https://modal.com/blog/comfyui-mem-snapshots); the page itself returned 404 today, so treat as unverified. Older post: [Scaling ComfyUI](https://modal.com/blog/scaling-comfyui).
- Image build: Python `Image` (pip/comfy-cli, `comfy node install`), custom nodes baked in the image. We write our own wrapper (no ready worker like RunPod's `worker-comfyui`): start ComfyUI server in `@modal.enter`, POST our workflow JSON to `127.0.0.1:8188/prompt`, poll `/history`, read the saved video. Our `AIVSSaveVideoBase64` node can be reused.
- Weights: one Volume, `.commit()` after seeding, `.reload()` on miss. v2 volumes recommended for new volumes (more files, throughput, many writers); v1 limit 500,000 inodes, best under 50,000 files; **design throughput up to 2.5 GB/s, not guaranteed**; no per-volume size cap stated [volumes](https://modal.com/docs/guide/volumes). Theory: 40 GB at 2.5 GB/s = ~16 s read; real value unmeasured.
- Alternative: S3/R2 `CloudBucketMount` (read-only possible; Mountpoint-based, sequential reads; no throughput numbers) [doc](https://modal.com/docs/guide/cloud-bucket-mounts).
- Cold start advice from Modal: download weights beforehand (tens of GB: minutes down to seconds), load independent files concurrently, move setup into `enter` [cold-start](https://modal.com/docs/guide/cold-start). Containers boot ~1 s.
- **Memory snapshots** [doc](https://modal.com/docs/guide/memory-snapshot): CPU snapshots are normal; **GPU snapshot is alpha** (`enable_memory_snapshot=True` + `experimental_options={"enable_gpu_snapshot": True}`), only for deployed apps, single-GPU/CUDA only, `torch.compile` may break creation, 2-3 snapshots per GPU type needed for full coverage, redeploy invalidates them, "do not speed up loading model weights from storage and can add overhead". Practical read: snapshots help ComfyUI/imports/CUDA init, not the 30-50 GB weight read. Wan 14B high+low noise models may need ~2 stages loaded in sequence; weights stay in RAM/VRAM only while the container lives.

### Beam
- Two official routes [example](https://docs.beam.cloud/v2/examples/comfy-ui.md), [blog](https://www.beam.cloud/blog/comfyui-api): (a) a `Pod` running the ComfyUI server on port 8000 (native `/prompt` API, public HTTPS URL, closes after 10 idle min by default); (b) an `@asgi` handler that starts ComfyUI in `on_start` and exposes `/generate` (deploy with `beam deploy api.py:handler`; outputs as signed URLs from `Output`).
- Example uses **RTX 4090**; weights baked into the image (Flux schnell, image ~23 GB, first build ~10 min). Measured/documented: Pod ComfyUI responds ~30 s after start, first prompt "a few minutes" while the model loads to VRAM; **endpoint first request after scale-from-zero ~90 s**, then seconds [example](https://docs.beam.cloud/v2/examples/comfy-ui.md).
- Custom nodes: `comfy node install` lines in the image; node must be in the image or the request fails.
- Weights: bake in image or a Beam Volume (`beam cp`). Volume size limit, region, and read throughput: **not documented** (unverified). Changes may take time to appear in other containers.
- Checkpoint/restore: `checkpoint_enabled=True` exists; "support depends on the runtime and workload"; status (beta/GPU support), limits, and timings **not documented** [cold-start doc](https://docs.beam.cloud/v2/topics/cold-start.md). Beam also claims 1-3 s container boots, vendor figure ([blog](https://www.beam.cloud/blog/cold-start-serverless-gpu-inference)). Beam notes cold start is dominated by weight loading, which snapshots do not remove.
- No published Wan/video example, no measured 30-50 GB load.

## 3. Job API from a desktop app/backend

| Need | Modal | Beam |
|---|---|---|
| Async submit | No built-in REST. Deploy a function, wrap `.spawn()` in our own `fastapi_endpoint`; `call_id` = job id [job-queue](https://modal.com/docs/guide/job-queue) | `@task_queue` deployed; `POST` returns `task_id`; also `.put()` from Python [tasks](https://docs.beam.cloud/v2/task-queue/running-tasks.md) |
| Status | Our own `GET /result/{id}` using `FunctionCall.get(timeout=0)` (202 while pending); results kept **7 days** | `GET https://api.beam.cloud/v2/task/{id}/` (Bearer); fields status/started_at/ended_at/outputs/stats; full status list not documented |
| Webhook | None documented; do the callback inside our function | `callback_url` on the decorator: Beam posts the task response when done |
| Time limits | Web request capped 150 s then 303 redirect to a result URL (~50 min in browsers); use spawn+poll instead [webhook-timeouts](https://modal.com/docs/guide/webhook-timeouts). Function `timeout` default configurable (example 10 min) | Endpoints hard cap **600 s** (default 180 s); task queue default 3600 s, `timeout=-1` disables [timeouts](https://docs.beam.cloud/v2/topics/timeouts-and-retries.md) |
| Retries | configurable `retries` | task queue default **3 retries (cap 5)**: a failed 2-min video can bill 4 times; set `retries=0` |
| File output | return bytes (web response body unlimited, request body up to 4 GiB per [webhooks](https://modal.com/docs/guide/webhooks) search result; gRPC payload ~100 MB per [troubleshooting](https://modal.com/docs/guide/troubleshooting) search result); better: write to R2/Volume and return a URL | `Output(path).save()`: signed URL, `expires_in` 3600 s in example; size limit not documented |
| Auth | Proxy tokens (`Modal-Key`/`Modal-Secret` headers) or our own Bearer check with a Modal Secret [webhooks](https://modal.com/docs/guide/webhooks) | Endpoint `Authorization: Basic`/Bearer workspace token; status uses Bearer |
| Per-user quotas | None native for end users. Do it in our backend (as in the RunPod note). Modal Team plan has environment-level budgets; Starter spend cap: unverified | Same: none documented |

Desktop app should never hold the provider token: same thin-backend design as the RunPod note applies to both.

## 4. Acceptable use (face swap / deepfake / adult)
- **Modal** ([terms](https://modal.com/legal/terms)): §1.4 bans illegal content, IP infringement, false statements, "indecent or obscene material", "invasion of privacy or misappropriation of publicity rights"; §1.5 bans promoting unlawful activity; customer is "solely responsible" for input and review of AI output (§5.5); suspension after notice (§1.6). Deepfakes, adult and impersonation are **not named**. A separate AUP page returned 404; none found. Adult content is the ambiguous part ("indecent or obscene"): treat as forbidden.
- **Beam** ([terms](https://docs.beam.cloud/v2/security/terms-and-conditions.md)): §3.2(c) bans content sexually exploiting minors, §3.2(d) defamatory/fraudulent/deceptive, §3.2(e) privacy/publicity violations, §3.3(g) impersonating any person "in a manner intended to deceive", §3.3(h) harassment. Adult content and deepfakes **not named**; suspension if not cured. No SLA ("uptime commitment only if in an order form"); liability cap = greater of 12 months fees or $100; US data location by default; no training on customer data.
- Both are looser on paper than RunPod (explicit adult ban + written consent of identifiable people, per the old note). Consent-attestation, NSFW filter and logging from the old note still apply; a face-swap service that produces non-consensual or sexual content would breach "privacy/publicity rights" and "deceptive impersonation" clauses on both.

## 5. Reliability signals
- **Modal** [status](https://status.modal.com): last 90 days listed incidents are all short (Sep 4 Volumes degraded 2 h 08; Sep 16 dashboard/sandbox down 14 min; Oct 5 function/sandbox degraded 7 min; Oct 19 web functions 2 min; Nov 10 sandbox creation 23 min; maintenance Nov 1 and **Nov 16 01:00 UTC**). Uptime 30/60/90 d: Functions 100%, Web Functions 100%, Volumes 100/99.997/99.998%. GPU capacity/queue incidents: none labelled (does not mean none).
- **Beam** [status](https://status.beam.cloud): all operational, no incidents Oct 30 to Nov 13 2026; no 90-day uptime numbers shown; no SLA. GPU availability and queueing: nothing independent found; Beam itself says to check `beam machine list` for live availability.
- Regions: Modal region pinning costs 1.15-1.75x; Modal spawn payloads are stored in centralized US storage ([data residency](https://modal.com/docs/guide/data-residency), search result). Beam default US storage.
- ComfyUI user stories: Modal has multiple first-party posts and community repos (e.g. community "modal-comfy-worker"); for Beam only vendor material found, no independent Reddit/GitHub reports (searched, none surfaced). Wan 2.2 14B on Modal/Beam specifically: no source.

## 6. Cost model (my arithmetic, not provider quotes)

Assumptions: 5 s Wan 2.2 14B video = **120 s on H100** (given). Other cards scaled by guess (unverified): A100 80GB 1.7x slower = 204 s; L40S 2.5x = 300 s (48 GB, may need offload); H200 same 120 s (could be ~1.3x faster); B200 0.6x = 72 s. 4090 uses the app's measured 76 s (lightning workflow, [[Vast protocol]]). 20 videos/day x 30 = 600/month. CPU/RAM: 4 vCPU-equivalent (2 Modal cores) + 32 GiB = $0.0000972/s on Modal; Beam 4 cores + 32 GiB at serverless-gpu rates = ~$0.00060/s (so Beam CPU/RAM is **larger than the 4090 itself**; a lean 2 core/16 GiB config halves it). Cold overhead: 5 cold sessions/day, each 90 s weight load + 60 s idle tail = 150 s billed GPU-time, 22,500 s/month (guess).

| Option | s/video | $/video (GPU + CPU/RAM) | 600 videos/mo | + cold overhead | Storage 150 GB | Idle total |
|---|---|---|---|---|---|---|
| Modal B200 | 72 | 0.125 + 0.007 = **0.132** | $79 | +$41 | $0 if 1 TiB free holds, else $13.50 | $0-13.50 |
| Modal H200 | 120 | 0.151 + 0.012 = **0.163** | $98 | +$31 | same | |
| Modal H100 | 120 | 0.132 + 0.012 = **0.143** | $86 | +$27 | same | |
| Modal A100 80GB | 204 | 0.142 + 0.020 = **0.162** | $97 | +$17 | same | |
| Modal L40S | 300 | 0.163 + 0.029 = **0.192** | $115 | +$15 | same | |
| Beam 4090 serverless (lean 2c/16G) | 76 | 0.0146 + ~0.017 = **~0.03** | ~$19 | +~$7 | planned $10.80, unbilled today | ~$0-11 |
| Beam 4090 serverless (4c/32G) | 76 | 0.0146 + 0.045 = **~0.06** | ~$36 | +~$14 | same | |
| Beam H100 on-demand, 1 h lease/session (5 sessions/day, guess) | 120 | n/a, hourly | **~$275** (5 x $1.83 x 30) | included | same | $0 if released |
| RunPod H100 ($5.89/h from old note, flex price unverified) | 120 | 0.196 | $118 | +$37 | $10.50 (volume $0.07/GB) | ~$10.50 |
| RunPod 4090 ($1.34/h, old note) | 76 | 0.028 | $17 | +~$5 | $10.50 | |

Reading: per video, Beam 4090 and RunPod 4090 are 4-5x cheaper than any large card on Modal at the assumed speeds. Large cards only win if they are much faster than assumed, or if 4090 cold start/VRAM (24 GB, Wan 14B fp8) proves troublesome. Idle storage for 150 GB: Modal $0-13.5, Beam $0 today ($10.80 planned), RunPod $10.50, all small next to compute at 20 videos/day. Modal credit offset: Starter $30/mo covers ~35% of the H100 run cost.

## 7. Verdict: Beam vs Modal vs RunPod for our case

| Criterion | Modal | Beam | RunPod (old note) |
|---|---|---|---|
| Large cards serverless | Yes, all, per second | **No** (4090/5090 only; rest hourly machines) | Yes (L40S, A100, H100) |
| 4090-class cheap tier | No | Yes, $0.69/h | Yes, $1.34/h |
| ComfyUI ready path | Official example, own wrapper | Official example (4090, Flux), own wrapper | Ready `worker-comfyui` |
| Async REST + webhook | Roll your own | Built in (task queue + callback) | Built in (/run, /status) |
| Weight storage | Volume, ~2.5 GB/s design | Volume, no numbers, storage not yet billed | Network volume, one DC |
| Snapshots | CPU ok, GPU alpha | Exists, undocumented limits | None; FlashBoot only |
| Maturity / status | Detailed status, short incidents | Thin docs, young status history | Larger user base |
| ToS for face swap | Neutral on paper | Neutral on paper | Explicit adult ban |
| Dev effort | Medium (wrapper + backend) | Medium | Lowest |

**Recommendation: Modal as the primary candidate**, because the stated preference for larger cards makes Beam structurally unsuitable, Modal's docs are the most complete (limits, preemption, snapshot caveats), the free $30 credit makes validation free, and its ToS is not more restrictive than the alternatives. Keep RunPod as the lowest-effort fallback (ready worker). Park Beam: revisit only as a cheap 4090 endpoint if Modal/RunPod per-video cost matters (at ~$0.03/video it is the cheapest) after the `gpu="H100"`-serverless question is answered.

Caveats that could flip it: Modal GPU jobs are preemptible and restart on the same input (billed again?; billing of a preempted run: unverified); no native webhook/REST; 10-GPU concurrency cap on Starter (fine for us); cold start for Wan unmeasured.

### Cheapest validation (ask the user before signing up; cap ~$3, Modal part probably $0 within credits)
1. Modal Starter account (project email), no card needed beyond what GPU use requires (docs say a valid payment method is required for GPU use), set a workspace spend limit if available.
2. Seed one v2 Volume with Wan 2.2 14B i2v (high+low noise fp8 or the current pack), VAE, umt5, lightx2v LoRA; run the download in a CPU-only function (~$0.1-0.3).
3. Image: our custom nodes + ComfyUI; class with `gpu="H100!"` (avoid silent H200 so the number is comparable), `scaledown_window=60`, `min_containers=0`; expose `spawn` + `result` endpoints with proxy auth.
4. Measure (about 8 runs x ~3 min x $0.0012/s = ~$1.5 total GPU, inside the $30 credit): (a) cold: request to first sampler step, split into container boot / weight read from Volume / ComfyUI boot / VRAM load; (b) warm 5 s i2v wall time on H100 (checks the 120 s assumption); (c) same after 10 min idle; (d) read throughput with `dd`/`cat` of a 20 GB file from the Volume; (e) repeat once with `gpu="L40S"` for $/video; (f) optional: enable CPU memory snapshot for ComfyUI start only and compare; (g) result size/latency for a ~10 MB mp4 returned via endpoint and via R2.
5. Gate: cold start <= 3 min and warm <= 2 min on H100, $/video <= ~$0.15 -> proceed with Modal. Else: run the RunPod test from the old note; consider Beam 4090 only after a $20 Developer-plan test (~$0.5: deploy ComfyUI `@asgi` endpoint with weights on a Volume, 3 runs, check the first-request time and whether `gpu="H100"` serverless is accepted).
6. Delete app, volume and tokens; confirm $0 running resources.

## Sources read (2026-11-13)
modal.com/pricing, modal.com/docs/guide (cold-start, memory-snapshot, volumes, gpu, resources, job-queue, webhooks, webhook-timeouts, cloud-bucket-mounts, preemption via search), modal.com/docs/examples/comfyapp, modal.com/legal/terms, status.modal.com; beam.cloud/pricing, beam.cloud/serverless-gpu, beam.cloud/blog/comfyui-api, docs.beam.cloud/v2 (introduction, examples/comfy-ui, topics/cold-start, topics/timeouts-and-retries, task-queue/running-tasks, data/volume, scaling/concurrency, scaling/pools, environment/gpu, resources/faq, security/terms-and-conditions), status.beam.cloud. Not readable (404): modal.com/legal/use-policy, modal.com/docs/guide/limits, modal.com/blog/comfyui-mem-snapshots, beam.cloud/terms.
