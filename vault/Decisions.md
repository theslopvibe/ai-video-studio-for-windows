---
tags: [ai-video-studio, decisions]
---
# Decisions (and why)

- **Tauri 2 + Svelte 5** — installer ~4 MB, low RAM, no Chromium bundled. User asked for lightweight. NSIS per-user (no admin), WebView2 `embedBootstrapper`.
- **No Python SDK** — Vast only ships Python; the protocol is plain HTTP so it is reimplemented in Rust. Friend's PC needs nothing installed.
- **Keys in Windows Credential Manager** (`keyring`), never in files, never sent back to the UI.
- **SQLite (rusqlite bundled)** for projects/jobs/settings; queue survives restarts.
- **Local save instead of R2** (user request) — impossible with stock template (only S3 URLs leave the worker). Solution: custom ComfyUI node returning base64 inline; installed by patching a *private copy* of the template. R2 kept as automatic fallback. Both unverified on a real worker ([[Known limits]]).
- **Always create a new private template, never edit the original** — public template can't be edited; keeps the user's original intact.
- **Prepend** the AIVS block in on-start (original may end in a foreground process, appended lines would never run).
- **Claude Code CLI as default enhancer** (user's subscription, no API key) with minimal flags; Gemini/OpenAI-compatible added so subscription tokens can be avoided entirely.
- **Per-project folder + style suffix + .txt sidecar** — user request ("prompt in the video" interpreted as saved prompt next to video; burn-in caption would need ffmpeg, not done).
- **Fastest mode is default** (sorted by `est_sec`).
- **Workflows are data** — drop JSON into the user folder; placeholders typed (`"{{width}}"` keeps number type).
- **Honest progress** — `/generate/sync` is blocking, so only phase + elapsed time, no fake %.
- **Light/dark** via CSS variables + `data-theme`.

## 2026-10-31
- Vast: one rented instance instead of serverless; deleted when idle (10 min) because a stopped one still bills ~29 USD/month for 110 GB.
- Extension (.mcpb) and Connect-Claude.bat ship inside the installer (resources/connect) and are explained in Settings; Claude Code uses `claude mcp add ai-video-studio -- "<app.exe>" --mcp`.
- One update per feature batch (user wish): test on a real card before releasing.

- **2026-11-13: RunPod Serverless replaces Vast** — per-second billing, models on a network volume (nothing downloaded per video), endpoint sleeps via workers.max 0 so nothing bills. Vast storage billed when stopped and re-downloads were slow.
- **Keys never leave the app** — API/MCP/logs never return them; Claude and ZCode talk only to the local API (127.0.0.1 + token).
- **No default spend limit** (user choice); optional `runpod_cap_usd`, settable only inside the app; unreadable value = 5 USD.
- **One button connects Claude Desktop and ZCode** (writes their config files) instead of extension file / Node.

- **Two RunPod slots instead of a profile list** (2026-11-13) — the need is exactly two accounts (owner + Antonis); slot 1 keeps the old setting names so nothing migrates. Only the app UI and `set_defaults` can switch; the key of the inactive one is never used.

