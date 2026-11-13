---
tags: [ai-video-studio, research, providers, face-swap]
---
# Research: Vast alternatives + video-to-video face swap

Read 2026-11-13. Every price below was read from the cited page on that date. "unverified" = not confirmed from a primary source. No money was spent, no accounts created. Context: [[Vast protocol]], [[Known limits]], [[Next steps]].

## TL;DR
- **Provider:** RunPod Serverless + `worker-comfyui` custom image + Network Volume, behind OUR thin backend (user never sees a provider key). Fallback: Modal (L40S, snapshots) if RunPod cold starts disappoint.
- **Face swap default:** Wan 2.2 Animate (Mix/replace mode), Apache-2.0, native ComfyUI nodes. Fallback: LTX-2.3 head-swap LoRA (licence = LTX-2 Community, unverified terms). ReActor/inswapper only as opt-in user-supplied, NOT bundled (non-commercial).
- **Validate first** with a ~$3 RunPod test before any code (section 5).

## 1. Providers

Per-second billing, scale to zero. "Storage" = ~100 GB persistent for models.

| Provider | GPUs / price read | Idle cost (no jobs) | 100 GB storage | Cold start / model cache | Start jobs from desktop app | Notes |
|---|---|---|---|---|---|---|
| **RunPod Serverless** | 4090 $1.34/h, 5090 $1.89/h, L40S $1.89/h, A100 80GB $2.99/h, H100 $5.89/h (single hourly price shown, flex vs active split not shown; third-party blogs show lower flex, e.g. 24GB $0.69/h: unverified) [pricing](https://www.runpod.io/pricing), [serverless](https://www.runpod.io/serverless) | $0 compute when scaled to zero; idle-timeout (default 5 s) is billed; cold-start model load is billed; image pull + cached-model download are not billed [docs](https://docs.runpod.io/serverless/pricing) | Network volume $0.07/GB-mo (<1 TB) = ~$7 [docs](https://docs.runpod.io/serverless/storage/network-volumes). Volume is tied to ONE datacenter, which restricts GPU availability; no auto sync between DCs | FlashBoot "sub-200 ms" is vendor claim, works best with steady traffic [docs](https://docs.runpod.io/serverless/endpoints/endpoint-configurations). Wan 14B load into VRAM from a volume: no measured number found (unverified; plan for 1-3 min). "Cached models" = only ONE HF model per endpoint, so not enough for Wan (several files) [docs](https://docs.runpod.io/serverless/endpoints/model-caching) | REST `POST https://api.runpod.ai/v2/<endpoint>/run` + `/status`, Bearer key [docs](https://docs.runpod.io/serverless/endpoints/send-requests). `worker-comfyui` takes ComfyUI API-format JSON directly [repo](https://github.com/runpod-workers/worker-comfyui) | Max workers default 3; request payload ~10 MB (/run) / 20 MB (/runsync) so use S3/R2 for video I/O; result retention 30 min; $0 balance with volume keeps billing storage and may delete the volume. Scoped (per-endpoint) API keys: unverified |
| **Modal** | L40S $0.000542/s (~$1.95/h), A100 80GB $0.000694/s, H100 $0.001097/s; no 4090 [pricing](https://modal.com/pricing) | $0 at zero; warm containers (scaledown_window default 60 s) are billed [docs](https://modal.com/docs/guide/cold-start) | Volume $0.09/GiB-mo with "1 TiB free" shown on page (ambiguous, unverified) | Containers boot ~1 s; weights in Volume or image; memory snapshots (GPU snapshot = alpha) claim 3-10x faster init but do NOT speed up weight loading from storage [docs](https://modal.com/docs/guide/memory-snapshot) | Python SDK/web endpoints; ComfyUI means writing our own wrapper (no ready worker) | Starter $30/mo free credits; better DX, higher $/s than RunPod 4090 |
| **Replicate** | L40S $0.000975/s, A100 $0.0014/s, H100 $0.001525/s [pricing](https://replicate.com/pricing) | Private models: you pay setup + idle + active time while instance online; "fast-booting fine-tunes" only billed when active [pricing](https://replicate.com/pricing) | Weights baked in Cog image; storage price unverified | Not stated | HTTP API; needs Cog packaging of ComfyUI | Most expensive per second; poor fit |
| **fal.ai** | H100 $4.50/h list (as low $2.49), H200, B200 [pricing](https://fal.ai/pricing) | Runner billed in SETUP/IDLE/RUNNING states; scale to zero; `min_concurrency`/`keep_alive` [docs](https://fal.ai/docs/serverless/introduction) | /data storage: unverified | Not stated | `fal.App` Python or custom Docker image; **serverless deploy needs fal team approval per account** | Great for managed model APIs; heavy for custom ComfyUI |
| **Baseten** | L4 $0.85/h, A100 $4.00/h, H100 $6.50/h [pricing](https://www.baseten.co/pricing/) | Scale-to-zero not described on pricing page (unverified) | unverified | unverified | Truss packaging | Priced high; no 4090 |
| **Beam** | RTX 4090 $0.69/h, 5090 $1.09/h [pricing](https://www.beam.cloud/pricing) | Billed by ms while code runs; cold start of container image not billed | Storage "not billed yet", planned $0.0001/GB-h (= ~$7.2/mo for 100 GB) | No latency numbers | Python SDK, Developer plan $20/mo incl. $20 credit | Cheapest data-centre 4090 found; small company, maturity unverified |
| **Cerebrium** | L40S $0.000542/s, A100 $0.000583/s, H100 $0.000944/s [pricing](https://www.cerebrium.ai/pricing) | $0 compute at zero | $0.05/GB-mo, first 100 GB free | "1-3 s scale" claim; snapshots "even faster" (vendor claims) | Python SDK/Docker; Hobby plan free + compute (5 concurrent GPUs) | Candidate; unproven for ComfyUI/Wan |
| **Salad** | RTX 4090 $0.16-0.44/h, 5090 $0.25-0.60/h by priority tier [pricing](https://salad.com/pricing) | Billed while running; allocation/image download free | **No persistent volume**: instance disk up to 250 GB but a replacement node starts empty; image limit 35 GB compressed [docs](https://docs.salad.com/products/sce/overview) | Models must be in image (Wan stack >35 GB: does not fit) or downloaded each time | Container Gateway + job queue | Consumer PCs, interruptions, variable network: same class of pain as Vast. Rejected |
| **Koyeb GPU** | L40S $1.20/h, A100 $1.60/h, H100 $2.50/h, L4 $0.70/h; no 4090 [pricing](https://www.koyeb.com/pricing) | Autoscaling + scale-to-zero, per second | Volume price not listed | Not stated | HTTP service/Docker | Cheap A100/H100; ComfyUI support, volumes and cold start unverified. Worth a second look |
| **Lambda** | H100 $3.79-4.89/h, A100 40GB $1.99/h (effective 2026-11-16) [pricing](https://lambda.com/pricing) | Instances only; **no serverless** on page | Filesystem price unverified | Boot a VM | Cloud API | Same "card" model as Vast; no help |
| **Comfy Cloud** (comfy.org) | pricing on comfy.org/cloud/pricing (not read) | Billed only while GPU runs | Preinstalled models | n/a | API exists, parallel jobs need paid plan; **cannot install own custom nodes**, fixed node packs [docs](https://docs.comfy.org/get_started/cloud) | Per-user subscription; our custom node (AIVSSaveVideoBase64) and workflows would not run. Rejected |
| Vast (baseline) | 4090 ~$0.4/h | stopped instance keeps GPU lost + disk $0.267/GB-mo ≈ $29/mo for 110 GB ([[Vast protocol]]) | | 10-27 min first video | | |

### Monthly estimate (my arithmetic, not provider quotes)
Assumptions: 20 videos/day x 30 days; measured warm generation 76 s on 4090 (memory.md); 5 cold starts/day x 90 s model load (guess, unverified); H100/A100 assumed ~40-50 s per video (guess). Compute seconds/month: 4090/L40S class 59,100 s; H100 37,500 s.

| Provider (GPU) | (a) idle, storage only | (b) 20 videos/day compute | (b) + storage |
|---|---|---|---|
| RunPod (4090 @ $1.34/h) | ~$7 | ~$22 | ~$29 |
| RunPod (H100 @ $5.89/h) | ~$7 | ~$61 | ~$68 |
| Modal (L40S) | $0-9 | ~$32 | $32-41 |
| Cerebrium (L40S) | $0 (first 100 GB free) | ~$32 | ~$32 |
| Beam (4090 @ $0.69/h) | ~$7 (planned rate) | ~$11 | ~$18 |
| Koyeb (L40S @ $1.20/h) | unknown | ~$20 | unknown |
| fal (H100 @ $4.50/h list) | unknown | ~$47 | unknown |
| Replicate (L40S private) | unknown | ~$58 + idle while online | unknown |
| Vast today | ~$29 (stopped, loses GPU) | ~$12 (0.4 $/h x 30 h) + 10-27 min setup each time | |

Per-video marginal cost on RunPod 4090: ~76 s x $0.00037 = ~$0.03 warm, ~$0.06 with a cold start. Storage dominates at low volume.

## 2. Business model: our backend vs user's own key

| Option | Pros | Cons |
|---|---|---|
| A. Our backend (one RunPod account, we pay; users get an install token / credits) | Zero setup for users: install, chat, video. Models/GPU preloaded; no card/key concept. One storage bill (~$7). | We carry cost + abuse risk + legal liability for face swap. Needs a small server, payments, support. |
| B. Bring-your-own RunPod key | No cost/legal exposure for us. Simple code (base URL swap). | Non-technical users must sign up, add card, create an API key, and the endpoint+volume (~100 GB, ~$7/mo) must be created in THEIR account: effectively the Vast pain again. Unacceptable for the target users. |
| C. Hybrid | A by default; B ("advanced: use my own RunPod key + endpoint id") for technical users and as a free-forever escape hatch | Two code paths (small: only base URL + token differ) |

Security: never ship our provider key in the installer or `.mcpb`; it is trivially extractable (the app is open-source MIT, and `app.exe --mcp` is local). All provider calls go through our backend, which holds the RunPod key. (Scoped per-endpoint RunPod keys: unverified, so the backend is the safe boundary.)

Cost/abuse control: per-install token issued by backend (device id + email/invite code), per-token daily and monthly caps (seconds of GPU), global monthly spend cap + RunPod account spend limit (default $80/h shown in [docs](https://docs.runpod.io/serverless/pricing), set lower), max workers = 1-3, max clip length/resolution, execution timeout, rate limit per token, kill switch. Credits via Stripe Checkout or invite codes; Ko-fi ([[Home]]) can fund a free tier but is manual.

**Recommendation: C, launched as A with invite-code/free-quota for the first users** (e.g. 10-30 videos/month free, then prepaid packs). Backend: one Cloudflare Worker (or similar) + KV/D1 for tokens/usage + R2 for input/output files (R2 already used as fallback in the repo; no egress fees per [Salad doc note](https://docs.salad.com/products/sce/overview), verify on R2 page). Backend is ~4 routes: `POST /job`, `GET /job/:id`, `POST /upload-url`, `GET /usage`.

## 3. Claude/MCP flow with zero "card" concept

User chat: "make a 5 s video of X from this image" -> Claude calls `generate_video` -> done. No `card_start`, `vast_running`, `vast_stop`, `auto_close`.

Tools after migration (MCP in `mcp/` and `app.exe --mcp`): `generate_video(workflow, params, inputs)` (blocks or returns job id), `job_status(id)`, `list_workflows`, `usage()` (credits left), plus existing local tools (library, editor). Remove card_start/vast_*.

API calls replacing `card.rs` provision/ready/auto_close:

| card.rs concern | Replacement |
|---|---|
| search offers, create instance, onstart download, wait `/object_info/AIVSSaveVideoBase64` | none. Endpoint exists permanently (scale to zero); models live on the network volume. |
| ready/progress | `POST /v2/<ep>/run` -> `GET /v2/<ep>/status/<id>` (states IN_QUEUE / IN_PROGRESS / COMPLETED). "Warming up" is just IN_QUEUE; Claude says "starting the GPU, first video takes ~2 min". |
| token proxy + pinned TLS cert + `aivs-card.local` | removed: HTTPS to our backend (normal cert), user token in header. |
| auto_close / destroy | removed: idle timeout (set 5-30 s) scales workers to zero. Optional "keep warm 10 min" = `idle_timeout` setting or temporarily `workersMin=1` (costs money, backend decides). |
| speed test / slow-machine memory / watchdog / parallel cards | removed; parallelism = max workers. |
| files in/out | inputs: upload to R2 via presigned URL, pass URL in workflow (worker-comfyui supports base64 `images`; limit 10-20 MB so use URLs for video); output: S3-configured worker returns `s3_url` ([repo](https://github.com/runpod-workers/worker-comfyui)). |

Cold start expectation to tell users: first video of a session 1-3 min (unverified), next ones ~76 s on 4090-class.

## 4. Video-to-video + face swap (ComfyUI, open source)

| Option | Files / size | VRAM | ComfyUI node | Licence (commercial?) | Quality / speed | Same endpoint? |
|---|---|---|---|---|---|---|
| **Wan 2.2 Animate 14B (Mix = replace character/face, Move = animate)** | `Wan2_2-Animate-14B_fp8_e4m3fn_scaled_KJ.safetensors` 18.4 GB (v2 variant 17.3 GB) in [Kijai repo](https://huggingface.co/Kijai/WanVideo_comfy_fp8_scaled/tree/main/Wan22Animate); also `clip_vision_h`, `wan_2.1_vae`, `umt5_xxl_fp8_e4m3fn_scaled`, lightx2v 4-step LoRA [Comfy docs](https://docs.comfy.org/tutorials/video/wan/wan2-2-animate) | not stated by sources; assume 24 GB with fp8 + block swap (unverified) | Native `WanAnimateToVideo` + ComfyUI-KJNodes + comfyui_controlnet_aux (DWPose) + SAM2 points editor [docs](https://docs.comfy.org/tutorials/video/wan/wan2-2-animate); Kijai WanVideoWrapper alternative | **Apache 2.0** (model card [HF](https://huggingface.co/Wan-AI/Wan2.2-Animate-14B)); fp8 repack also Apache-2.0 | Replaces the whole character (body+face) with the reference image, motion/expressions from the video; ~4.8 s per 77-frame block, chain "Video Extend" for longer. Face-only fidelity < inswapper-style swaps; no speed benchmark found (assume 2-5 min per 5 s on a 4090: unverified). Width/height multiples of 16 | **Yes**: same Wan family, shares VAE/text encoder/lightx2v with current packs |
| LTX-2.3 head-swap LoRA (`ltx23-head_swap_v3`, by Alissonerdx) | LTX-2.3 22B distilled fp8, gemma-3-12B fp8, VAEs, LoRA (sizes not given) [Comfy hub workflow](https://comfy.org/hub/workflows/video-face-swap-bed989744195/) | not stated | VHS, KJNodes, ReservedRegionFrameComposer | LTX-2 Community License ([HF](https://huggingface.co/Lightricks/LTX-2.3)); LoRA licence not stated: **unverified, check** | Dedicated head/face swap, includes audio path, 8 steps | Yes (LTX already a target), adds ~30+ GB |
| ReActor + inswapper_128 | inswapper_128.onnx ~0.5 GB, buffalo_l, GFPGAN | <4 GB, runs on any card | [ComfyUI-ReActor](https://github.com/Gourieff/ComfyUI-ReActor) (GPL-3.0 code; has NSFW detector) | **Models non-commercial research only** (buffalo_l, inswapper_128, CodeFormer flagged; GFPGAN Apache 2.0) [ReActor README](https://github.com/Gourieff/ComfyUI-ReActor), [insightface](https://github.com/deepinsight/insightface) (code MIT; models NC; inswapper needs licence from contact@insightface.ai) | Fast (frame-by-frame, seconds-minutes), 128 px swap, soft faces, flicker | Yes, but licence blocks bundling/distributing/hosting it in a paid or shared service |
| FaceFusion | multiple onnx models | low | not a ComfyUI node (standalone app) | Code OpenRAIL-AS (badge; text in LICENSE.md not read) [repo](https://github.com/facefusion/facefusion); model licences unverified | Good, popular | Separate app, different integration |
| LivePortrait v2v | small onnx/pt | low | KJ LivePortrait nodes | Code licence not read; default InsightFace detector (NC) with a MediaPipe alternative [repo](https://github.com/KwaiVGI/LivePortrait) | Transfers expression/head motion onto a portrait (reenactment), not true face identity swap | Yes |

**Default: Wan 2.2 Animate Mix mode.** Reasons: Apache-2.0 weights (matches MIT open-source distribution and hosting for others), same ecosystem/files as existing Wan packs, official ComfyUI template, one endpoint. **Fallback: LTX-2.3 head-swap LoRA** (re-check LoRA + LTX-2 licence first) or, if speed/identity matter more, an opt-in "advanced" ReActor pack the user installs themselves with an explicit non-commercial notice (same stance as in [[Features]]: do not bundle). Short clips (<10 s) fit: 2 chained blocks ≈ 9.6 s.

Minimal workflow outline (from [Comfy docs](https://docs.comfy.org/tutorials/video/wan/wan2-2-animate)): LoadVideo (driving) + LoadImage (new face/character) -> DWPose estimator (pose + face crops) + SAM2 points on the first frame (character mask; needs a click: for Claude, auto-mask via SAM2 "person" prompt, see existing click-cut-out tool) -> UNETLoader (Animate fp8) + lightx2v LoRA + CLIPVision + umt5 + VAE -> `WanAnimateToVideo` (Mix: background_video + character_mask connected) -> KSampler 4 steps -> VAEDecode -> `AIVSSaveVideoBase64`. Extend groups for >4.8 s. Test on a single GPU first; sizes multiples of 16.

### Consent, safety, legal (not legal advice)
- **EU AI Act Art. 50**: applicable from 2 Aug 2026; providers must mark generated video machine-readably, deployers must disclose deepfakes; Code of Practice final 2026-06-10, voluntary [EC page](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content). Our app is a provider of the generation system if we host it: embed an "AI-generated" metadata tag (mp4 metadata / C2PA, check C2PA tooling) and show a visible label option on face-swap outputs.
- **Wan licence** makes users "fully accountable" and forbids unlawful/harmful content (Apache 2.0 + usage note [HF](https://huggingface.co/Wan-AI/Wan2.2-Animate-14B)).
- **Provider ToS**: RunPod bans pornography/adult content (lifetime ban risk) and requires written consent of every identifiable person in content, no impersonation [terms](https://www.runpod.io/legal/terms-of-service). Hosting face swap on our RunPod account puts OUR account at risk. fal, Modal, Koyeb ToS not read: unverified.
- Safeguards to add: face-swap tool disabled until user ticks "I have the consent of the person whose face I use; no minors, no sexual content, no deception"; store the attestation + job hash locally and in backend logs; NSFW/nudity classifier on inputs and outputs (server side) and block public-figure/celebrity matches if feasible; no minors check (reject if face-age classifier says child, if a reliable one exists: unverified); watermark/metadata on outputs; per-token rate limit and ban list; takedown contact; short retention of uploads (delete R2 files after N hours); Greek/English terms and a clear "do not impersonate or defraud" notice. Optionally keep face swap off for free-tier tokens.

## 5. Recommendation, migration, risks, validation

**Pick: RunPod Serverless (flex, scale to zero) running a custom image `FROM runpod/worker-comfyui:<ver>-base` with our custom nodes, models on one Network Volume (~100 GB, ~$7/mo), fronted by our own backend (option C).** Why: only candidate with a ready ComfyUI worker taking our existing API-format JSON, 4090/5090 class prices, volume storage at $0.07/GB, no idle compute, per-second billing, plain REST for a desktop app. Modal is the technical runner-up (better snapshots, but no 4090, own wrapper to write, pricier). Beam is cheapest but least proven; Koyeb worth a check.

### Migration (mapped to files)
- `app/src-tauri/src/vast.rs`: replace with `cloud.rs` client for OUR backend (`/job`, `/job/:id`, `/upload-url`, `/usage`); keep a `runpod` direct mode (base URL `https://api.runpod.ai/v2/<ep>`, user key) as the advanced path.
- `app/src-tauri/src/card.rs`: delete provision/offers/onstart/proxy/TLS pin/token/auto_close/speed-test/watchdog; keep only "submit workflow, poll, download result", reusing the existing workflow-prep and `AIVSSaveVideoBase64` output logic (or switch to `SaveVideo` + S3 URL).
- New repo folder `docker/` (image: worker-comfyui base + custom nodes + `extra_model_paths.yaml` pointing at `/runpod-volume`) and `scripts/seed-volume` (one-time download of Wan/FLUX/LTX/Animate files to the volume through a temporary pod).
- New `backend/` (Cloudflare Worker): token issue, quotas, RunPod key holder, presigned R2 URLs, content filter hook, spend cap.
- `mcp/` server + `app.exe --mcp` (mcp.rs): drop `card_start`, `vast_running`, `vast_stop`; add `generate_video`/`job_status`/`usage`; bump `.mcpb` manifest; both MCP servers + .mcpb must be updated (see memory: parallel cards v0.8.0).
- Settings UI: replace Vast key section with "Account / credits" + advanced "own RunPod key"; remove card page and card auto-close options; update Greek README/ΟΔΗΓΙΕΣ.
- Workflows: add `wan22-animate-replace` pack (+ attestation gate), keep existing packs; update model list for the volume.
- Docs: [[Vast protocol]] mark superseded; [[Known limits]] add new unverified items; [[Decisions]] record the model.

### Risks / unknowns
1. Real cold start for Wan 2.2 14B (+ Animate) from a network volume: unmeasured; may be 1-3+ min (RunPod volume read speed unverified). Mitigations: fp8 weights, models on volume, FlashBoot (needs steady traffic), optional "warm for 10 min" (costs ~$0.22 per 10 min on a 4090 at $1.34/h), or bake models into image.
2. Single-datacenter volume limits GPU availability; may need volumes in 2 DCs (double storage ~$14).
3. RunPod flex vs active price: page shows one hourly price; confirm in console.
4. Wan Animate VRAM/speed on 24 GB: unmeasured (use 5090 32 GB / L40S 48 GB if needed).
5. All current workflows beyond Wan i2v are untested on real ComfyUI (memory.md).
6. Payload limits (10/20 MB) and 30-min result retention: need R2 path for video in/out.
7. Provider ToS exposure for face swap; hosted-by-us legal liability (EU AI Act, deepfake laws, GDPR for uploaded faces).
8. Abuse economics: someone loops jobs; need hard caps before launch.
9. Scoped API keys at RunPod: unverified; keep key server-side only.
10. Account balance reaching $0 can delete the volume: keep auto-top-up and alert.

### Cheap validation (ask the user before spending; cap about $3-5)
1. Create a RunPod account (project email), add $5 credit, set spend limit low.
2. Create a 100 GB network volume in one DC (~$7/mo, prorated about $0.25 for a day) and seed it using the cheapest pod (~$0.3-0.7/h x ~1 h download) with Wan 2.2 i2v lightning + Animate fp8.
3. Deploy serverless endpoint from the stock `worker-comfyui:-base` + our custom node pack, flex, max workers 1, idle timeout 5 s.
4. Measure: (a) first request cold start to first sample, (b) second request warm time, (c) after 10 min idle again, (d) 5 s i2v, (e) 5 s Animate replace, (f) volume read throughput. Budget: ~6 runs x ~3 min x $0.00037/s ≈ $0.4 compute + ~$0.7 pod + volume ≈ under $3.
5. Decision gate: cold start <= 3 min and warm <= 2 min for i2v on 4090/5090 -> proceed. Else repeat step 4 on Modal L40S (Starter plan $30 free credits, so $0) before deciding.
6. Delete endpoint/pod; keep volume only if proceeding; verify $0 running resources.

## Sources read (2026-11-13)
runpod.io/pricing, runpod.io/serverless, docs.runpod.io (serverless/pricing, endpoint-configurations, model-caching, network-volumes, send-requests), runpod.io/legal/terms-of-service, github.com/runpod-workers/worker-comfyui, modal.com/pricing + docs (cold-start, memory-snapshot), replicate.com/pricing + docs, fal.ai/pricing + docs, baseten.co/pricing, beam.cloud/pricing, cerebrium.ai/pricing, salad.com/pricing + docs, koyeb.com/pricing, lambda.com/pricing, docs.comfy.org (cloud, wan2-2-animate), huggingface.co (Wan-AI/Wan2.2-Animate-14B, Kijai/WanVideo_comfy_fp8_scaled, Lightricks/LTX-2.3), comfy.org hub video face swap, github (ComfyUI-ReActor, insightface, facefusion, LivePortrait), digital-strategy.ec.europa.eu code of practice.
