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
