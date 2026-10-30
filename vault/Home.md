---
tags: [ai-video-studio, index]
---
# AI Video Studio — Home

Windows desktop app (Tauri 2 + Rust + Svelte 5): prompt → mp4 using **Wan 2.2** on **Vast.ai Serverless** (ComfyUI). Scales to zero, saves videos locally. UI Greek/English, dark/light.

**Status (2026-10-29):** v0.1.0 built, installer on Desktop. 48/48 e2e checks pass against a *mock* Vast. **No real generation yet** (no keys were available). Next: [[Next steps]].

## Map
- [[Architecture]] — modules, data flow, where state lives
- [[Vast protocol]] — route → worker, template/workergroup APIs, what is verified
- [[Features]] — every UI page and the code behind it
- [[Decisions]] — why Tauri, why local-save node, why Claude Code CLI, etc.
- [[Testing]] — unit tests, mock server, CDP e2e
- [[Known limits]] — what is NOT verified
- [[Dev how-to]] — build, run, paths, pitfalls
- [[Next steps]] — first real run checklist and backlog

Repo root: `E:\claude\video-generator` (`app/` code, `dist/` installer, `PLAN.md`, `README.md` (Greek, user-facing), `memory.md` (handoff)).


## Open source (2026-10-30)
Δημόσιο repo: https://github.com/theslopvibe/ai-video-studio-for-windows · Release v0.1.0 με installer · Donate: https://ko-fi.com/slopvibe · Developer: theslopvibe.com · Site tab: https://theslopvibe.com/free-apps · Επόμενα: [[Next steps]], `ROADMAP.md`.
