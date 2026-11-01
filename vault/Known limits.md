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

12. v0.5.0 tools (voice, background removal, upscale, Qwen edit, RIFE, LatentSync, chaining at run time) were written from node source code by research agents and never run: expect fixes to node/input names on the first real run (`SaveVideo` format/codec keys, `LoadAudio` decode, Kokoro/kokoro-onnx version, Chatterbox VRAM on 8 GB). Local ComfyUI + models were deleted on request: re-run the wizard first.
13. Image editor AI menu: checked only that the servers answer, not clicked in a real window.

14. Security audit (Fable subagent, 2026-10-30) fixed in 0.5.3: no insecure TLS retry to workers; API Host check + constant-time token + empty 404; image_path via stash_input; editor postMessage origin check; CSP + runtime asset scope (`sync_asset_scope`); nodes pinned by `rev` in models.json; user HF packs only .safetensors/.gguf; imported workflows cannot reuse bundled ids; chat prompt sanitises workflow text. Open: bundled pickle weights (chatterbox .pt, RealESRGAN .pth) and ComfyUI/7zr downloads have no checksum yet; Gemini CLI runs with default tools; installer/.mcpb built locally, unsigned (build in CI later).

15. v0.6.0 tools untested: Whisper node (torchaudio resample, ctranslate2 CUDA libs), SAM2 `coordinates_positive` forceInput STRING literal, LaMa/face helper nodes written from a sketch (GFPGAN normalisation (x-0.5)/0.5 assumed), TRELLIS.2 on 8 GB, DA3 dynamic-combo keys (`output.normalization`), DDColor timm import path. Click-to-point in miniPaint maps the click through the canvas rect (zoom/scroll not verified).

## 2026-10-31
- Card setup still 10-27 min (models download from HuggingFace). Idea: own docker image with models baked in (needs a build machine with Docker + 120 GB, registry GHCR/Docker Hub, licences re-checked); gain maybe 5-10 min, not instant; only for wan22 i2v/t2v packs.
- Connection card ↔ app is TLS with a self-made pinned certificate (0.7.8). Cards created by 0.7.7 (http) are not reused and are closed as leftovers.
- Stop-instead-of-delete not implemented. Auto-update flow never tested on a real second PC.
- Launching the app for tests: use PowerShell Start-Process; a Bash `&` child dies when the tool call goes to background.
