---
tags: [ai-video-studio, architecture]
---
# Architecture

```
Svelte UI (app/src) ──invoke──▶ Rust commands (src-tauri/src/lib.rs)
        ▲  events "job-update"          │
        └───────────────────────────────┤
                                        ├─ db.rs        SQLite (projects, jobs, kv settings)
                                        ├─ secrets.rs   Windows Credential Manager (service "AIVideoStudio")
                                        ├─ queue.rs     background loop: one job at a time
                                        ├─ vast.rs      HTTP client for Vast (route, worker, console API)
                                        ├─ workflows.rs load + fill ComfyUI workflow JSON
                                        ├─ provision.rs worker install script + template patching
                                        ├─ anthropic.rs Claude Code CLI + Anthropic API
                                        └─ llm.rs       Gemini + OpenAI-compatible
```

## Job lifecycle (queue.rs `run_job`)
1. status `queued` → loop picks oldest.
2. `routing`: `POST /route/` polled with backoff until a worker `url` arrives (cold start 1-3 min; gives up at 20 min). `error_msg` in a 200 reply = bad key/endpoint ([[Vast protocol]]).
3. `generating`: `POST <worker>/generate/sync` (blocking, no progress %). UI shows elapsed time.
4. delivery: **local** (default) swaps `SaveVideo` for node `AIVSSaveVideoBase64` (id `9999`) and reads base64 from `comfyui_response`; if worker says the node is missing → automatic retry in **cloud** mode (S3/R2 `output[].url`, downloaded).
5. file saved in project folder, optional `.txt` sidecar, `est_cost = dph_estimate × seconds / 3600`.
6. `done` / `failed` / `cancelled`. Jobs interrupted by app restart return to `queued` (db.rs open).

## State
- `%APPDATA%\com.aivideostudio.app\studio.db` (tables `kv`, `projects`, `jobs`), `workflows\` (user workflows), `provisioning`-related none stored.
- Keys: Credential Manager only (`vast_api_key`, `anthropic_api_key`, `gemini_api_key`, `openai_api_key`). Never returned to UI (`secrets::status` returns booleans).
- Settings (kv): see `SETTING_KEYS` in lib.rs: lang, theme, endpoint_name, out_dir, enhance_provider (`cc|api|gemini|openai`), claude_model, anthropic_model, gemini_model, openai_base_url/model, delivery (`local|cloud`), gpu_selection, dph_estimate, last_project, last_workflow, models_selected, worker_pack.

## Workflows as plugins
`app/src-tauri/workflows/*.json` (bundled) + user dir. Format `{meta, workflow}` with `"{{key}}"` placeholders, or a plain ComfyUI API export (prompt injected into the positive `CLIPTextEncode`). User file with same `meta.id` overrides bundled. Sorted fastest-first (`est_sec`).

## Frontend (app/src/lib)
`store.svelte.ts` (runes store `S`), `api.ts` (typed invoke), `i18n.ts` (el/en dictionary), pages: Generate, Queue, Library, Projects, Gpu, Models, Settings + `JobCard`. SvelteKit static SPA (`ssr=false`).
Svelte 5 gotcha: never write `$state` inside a template expression (broke the Projects page once) → init in `$effect`.
