---
tags: [ai-video-studio, features]
---
# Features → code

| Feature | UI | Backend |
|---|---|---|
| Generate (mode, prompt, size presets, duration, advanced, batch) | `Generate.svelte` | `job_enqueue` (applies project style suffix) |
| Improve prompt (✨, undo, shows tokens) | `Generate.svelte` | `enhance_prompt` → `run_llm` in lib.rs |
| Queue (phases, cancel, retry) | `Queue.svelte`, `JobCard.svelte` | `queue.rs`, `job_cancel`, `job_retry` |
| Library (grid, player, reuse, delete, open folder) | `Library.svelte` | `jobs_list`, `job_delete`, `reveal` |
| Projects (own folder, style suffix, .txt sidecar) | `Projects.svelte` | `project_create/update/delete`, `AppState::project_dir` |
| GPU & cost (live $/h, pick GPUs) | `Gpu.svelte` | `gpu_offers`, `apply_gpus`, `merge_gpu_filter` |
| Models + install on workers | `Models.svelte` | `install_on_workers`, `provision.rs`, `models.json` |
| Settings (keys, providers, delivery, theme, language, out dir, export/import) | `Settings.svelte` | `secret_set`, `setting_set`, `test_vast`, `test_llm`, `apply_scale_to_zero` |

## Prompt-improvement providers
`cc` Claude Code CLI (default; `claude -p --system-prompt … --tools "" --no-session-persistence --setting-sources "" --model haiku --output-format json`, prompt via stdin, ~0.9k tokens/call, floor ≈ 685 input tokens fixed by CLI), `gemini`, `openai` (any OpenAI-compatible base URL), `api` (Anthropic). Short system prompt `anthropic::SYSTEM` (paid on every call).

## Model catalog (`models.json`, URLs HEAD-verified 2026-10-29)
wan22-t2v (in template), wan22-i2v, wan22-ti2v-5b, flux1-schnell, sdxl-base. Entry may hold `nodes:[{repo,name}]` (cloned + pip requirements by the install script).

## Worker install script (`provision.rs`)
gzip+base64 block prepended after the template's shebang, between `# >>> AIVS BEGIN` / `# <<< AIVS END` (idempotent). Writes `custom_nodes/aivs_save_b64/__init__.py` (`aivs_save_b64.py`), downloads model files with `dl()`, clones nodes. Template copy gets bigger `recommended_disk_space` (≥ 100 + model GB), name prefix `AIVS `, workergroup repointed.

## Claude Desktop (MCP) — 2026-10-30
Τοπικό API (`api.rs`) + extension `mcp/ai-video-studio.mcpb`. Το Claude Desktop ζητά video, μοντέλο (ltx/wan) και κάρτα με ελεύθερο κείμενο· κάθε παραγωγή θέλει επιβεβαίωση κόστους. Λεπτομέρειες στο memory.md.

## Image editor + chained media tools (v0.4.0 / v0.5.0, 2026-10-30)
- **Image editor** = miniPaint (MIT) pinned at a79733e, built by `editor/build_paint.py` (+ `editor/paint/ai.js` = "AI" menu) into `app/src-tauri/paint-dist`, served by `editor.rs` on 47617 (video editor 47616). Editor page has Video/Image tabs. The AI menu talks to the app by `postMessage` (Editor.svelte `handleAi`): every entry is just a workflow (generate=best t2i, `rmbg-birefnet`, `upscale-realesrgan`, `qwen-image-edit`), result read back via `image_data_url`.
- **Chaining**: input types `image|audio|video`; param value `job:<id>` = result of that job (resolved at enqueue if done, else at run time by `queue.rs` because the queue is FIFO), `file:` = stashed upload (`stash_input`, ext whitelist, 600 MB). `input_import` command, API `inputs` map, MCP `generate_video.inputs`, chat blocks with `id`/`inputs:{slot:"@a"|"@last"}` ("Run all steps"). UI: Library "Make more from this…" and Create tabs Voice/Tools both use `Chain.svelte` (options = workflows whose input slot type matches).
- **New workflow kinds**: `tts`, `vc`, `i2i`, `v2v`, `lipsync`. Meta fields `languages`, `notice_en/el`; input field `options` (dropdown). Pack file `dir` may start with `@custom_nodes/` (weights next to node code).
- **aivs nodes** (`aivs_save_b64.py`, refreshed on every local start): AIVSLoadImage/Audio/VideoBase64.
- **Workflows added (all UNTESTED on real ComfyUI)**: chatterbox-tts/clone/vc (filliptm/ComfyUI_Fill-ChatterBox, MIT, 23 languages incl. Greek, Perth watermark not installed), kokoro-tts (stavsap/comfyui-kokoro, CPU, 9 languages, no Greek), rmbg-birefnet (lldacing/ComfyUI_BiRefNet_ll), upscale-realesrgan (core nodes), qwen-image-edit (GGUF Q4_K_M, ~23 GB, city96/ComfyUI-GGUF), interpolate-rife (Fannovel16/ComfyUI-Frame-Interpolation, requirements-no-cupy.txt), lipsync-latentsync (ShmuelRonen wrapper, needs ~18 GB VRAM, deletes ComfyUI temp dir, OpenRAIL++ weights; experimental).
- **Deliberately excluded**: face swap (InsightFace models non-commercial), CodeFormer, Wav2Lip, F5-TTS, MMAudio, Stable Audio Open, 4x-UltraSharp, SUPIR, Flux Kontext/Fill (gated/NC). Offer "bring your own model" only if ever added.

## v0.6.0 tools (2026-10-30) — all UNTESTED on real ComfyUI
- Output kinds: `subs` (Whisper → .srt/.vtt, node `AIVSTranscribeSubtitles` in `aivs_save_b64.py`, faster-whisper==1.2.1 via pack `pip`, output key `subtitles`; Library shows the text via `text_output`), `i2m` (TRELLIS.2 → .glb via core SaveGLB, output key `3d`, needs recent ComfyUI, ~8 GB).
- Helper nodes (ours, MIT): `AIVSPointsToJson`, `AIVSLamaErase` (big-lama.pt in models/inpaint, crop-around-mask), `AIVSFaceRestore` (facexlib + spandrel GFPGAN; facexlib installed `--no-deps` via pack `pip_nodeps`; weights licence grey).
- Workflows: whisper-subtitles(+audio), cutout-sam2, erase-object (SAM2 kijai pinned `0c35fff` + LaMa), face-restore-gfpgan, colorize-ddcolor (kijai DDColor pinned), depth-da3 (core DA3), trellis2-image-to-3d. Image editor AI menu: click-to-point tools (`pick_point` in `editor/paint/ai.js`), faces, colorize, depth.
- Pack fields: `pip`, `pip_nodeps` (strict `name==version`, `safe_pip_spec`), node `rev` (pinned commit), file `dir` starting `@custom_nodes/`.
