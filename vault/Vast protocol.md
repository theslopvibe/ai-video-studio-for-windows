---
tags: [ai-video-studio, vast, protocol]
---
# Vast.ai protocol (as implemented in `vast.rs`)

Sources: docs.vast.ai (comfyui-wan-2.2, route, endptjobs, workergroups, template), `vastai` SDK 1.9.1, `vast-ai/pyworker` (`workers/comfyui-json/worker.py`).

## Inference
1. `POST https://run.vast.ai/route/` `Authorization: Bearer <key>` body `{endpoint, api_key, cost:100, request_idx, replay_timeout:60}`.
   - reply with `url` ⇒ worker ready; **whole body is `auth_data`**.
   - reply without `url` ⇒ keep polling, reuse returned `request_idx`.
   - **bad key/endpoint ⇒ HTTP 200 `{"error_msg": ...}`** (found in testing; treated as error).
2. `POST <url>/generate/sync` (Bearer) body `{auth_data, session_id:null, payload:{input:{request_id, workflow_json}}}`.
3. Reply: `status`, `output[].url` (pre-signed, only if S3 configured), `comfyui_response` (history incl. node outputs), `execution_details.progress_updates`.
4. `"__RANDOM_INT__"` in the workflow is replaced server-side with a random seed.
5. Worker TLS uses Vast root CA (`https://console.vast.ai/static/jvastai_root.cer`). rustls may reject it → client falls back to default roots; on TLS cert error retries once with invalid certs accepted (request still signed). **Unverified on real worker.**
6. pyworker exposes only `/generate/sync` → no way to fetch a file from the worker except S3 URL or inline response.

## Console API (`https://console.vast.ai`, Bearer)
- `GET /api/v0/endptjobs/` → `results[]` (`id, endpoint_name, cold_workers, min_load, max_workers, endpoint_state`); `PUT /api/v0/endptjobs/{id}/` `{cold_workers:0,min_load:0}` = scale-to-zero.
- `GET /api/v0/workergroups/` (match `endpoint_id`/`endpoint_name`), `PUT /api/v0/workergroups/{id}/` `{search_params, gpu_ram}` (GPU choice, `gpu_name in [A,B]`) or `{template_hash}`.
- `POST /api/v0/bundles/` marketplace offers → grouped per GPU (`dph_total`).
- Templates: `GET /api/v0/template/?select_cols=["*"]&select_filters={"hash_id":{"eq":H}}` → `templates[]`; `POST /api/v0/template/` creates (we always create a **new private** copy, never edit the original); response parsed for `hash_id`.
- Env var overrides for testing: `AIVS_VAST_RUN_URL`, `AIVS_VAST_CONSOLE_URL`.

## Billing facts (docs)
Ready/Loading workers billed for GPU; Inactive = storage only. Scale-to-zero needs `cold_workers=0`, `min_load=0`.

## Wan 2.2 workflow notes
Doc example has **no LoRA**. Our Lightning workflow adds `LoraLoaderModelOnly` on both UNETs (high/low LoRA files shipped by template), shift 5, 4 steps, cfg 1, split 2/2, `KSamplerAdvanced` pair. Full workflow: 20 steps, cfg 3.5, shift 8, split 10. Ids: 90 CLIP, 91 neg, 92 VAE, 99 pos, 101/102 UNET, 104 latent, 95/96 samplers, 97 decode, 100 CreateVideo, 98 SaveVideo.
