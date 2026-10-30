---
tags: [ai-video-studio, testing]
---
# Testing

## Unit (Rust) — `cd app/src-tauri && cargo test` → 6 pass, 1 ignored
workflows fill/clamp + plain export, provision patch idempotency + script content, GPU filter merge, queue local-mode swap/inline detect. Ignored: `anthropic::tests::cli_enhance_end_to_end` (needs logged-in Claude Code; run `-- --ignored`; last run: 906 tokens, works via `claude.cmd` shim).

## E2E (48 checks) — `app/tests/e2e`
`mock.mjs` fakes Vast (route cold-start, worker inline + URL modes, console/template/workergroup APIs, 401 for key `bad`). `e2e.mjs` spawns the **release exe** with `AIVS_VAST_*_URL` + `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, drives it over CDP (invoke + DOM clicks + screenshots). Covers: first run, Test Connection, projects with folders/suffix/sidecar, UI-driven generate, 3-prompt batch order, fail/retry, cancel, missing-node fallback to URL, cloud mode, GPU apply, scale-to-zero, template install, every page in dark+light with content markers.
Run recipe in `tests/e2e/README.md`. It uses the real `%APPDATA%\com.aivideostudio.app` DB → delete that dir afterwards. Last result 48/48.

## Bugs the tests found (all fixed)
1. Worker client `build()` failed ("builder error") when Vast root CA couldn't be loaded → every generate would fail.
2. Vast returns HTTP 200 `error_msg` for bad key → would have polled 20 min.
3. Projects page silently not rendering (state write in template).
4. Radio inputs stretched by global `input{width:100%}`.

## Not covered
Real Vast, real GPU output, real template create/patch, Gemini/OpenAI/Anthropic API calls, installer on a clean PC, WebView2 bootstrapper path.
