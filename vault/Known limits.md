---
tags: [ai-video-studio, risk]
---
# Known limits / unverified

**Never run against real Vast.** Everything below is from docs/SDK, tested only on the mock.

1. **Inline local save** (`AIVSSaveVideoBase64`): assumes the API wrapper tolerates a workflow without `SaveVideo`, that ComfyUI history `outputs` keep our `ui.aivs_video_b64` key, that the wrapper returns it in `comfyui_response`, and that `video.save_to(path)` works with defaults. If any fails → automatic fallback to R2 (needs `S3_*` Vast env vars) with the Vast error shown.
2. **Template install**: field names of `GET /template/` assumed equal to create payload; `POST /template/` response assumed to contain `hash_id`; workergroup assumed to carry `template_hash`. Prepend position assumes ComfyUI starts after on-start lines.
3. **Lightning LoRA wiring** is ours (doc has none). Check output quality; fallback = "Quality (20 steps)".
4. **Worker TLS** (Vast root CA vs rustls) — fallback accepts invalid certs once; request still carries signed `auth_data`.
5. **Cost estimates**: `est_sec` (70 / 240) are guesses for RTX 4090-class; real cost per clip stored in `est_cost` after each run (needs a GPU selected so `dph_estimate` > 0).
6. No live progress % (blocking `/generate/sync`).
7. I2V workflows (`wan22-i2v-lightning`, `wan22-ti2v-5b`) and node `AIVSLoadImageBase64` written from ComfyUI docs, never run on a real worker (node names/inputs: `WanImageToVideo`, `Wan22ImageToVideoLatent`). Custom HF models need a user-supplied workflow; diffusers-format repos cannot load in ComfyUI.
8. `search_params` string format for GPU filter (`gpu_name in [A,B]`) assumed from CLI syntax.
9. Installer is unsigned → SmartScreen warning. Greek NSIS language not translated by Tauri (English installer UI).
10. jCodeMunch can't index `.svelte`; skips `secrets.rs` by filename.
11. Claude Code CLI needs login once; floor ≈ 0.7k input tokens per call.

- MCP/API: δοκιμασμένο μόνο χωρίς πραγματικό job· `resolve_gpu` δεν δοκιμάστηκε με πραγματικό Vast key· το `.mcpb` δεν έχει δοκιμαστεί σε Claude Desktop.
