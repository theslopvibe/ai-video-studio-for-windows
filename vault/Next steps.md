---
tags: [ai-video-studio, todo]
---
# Next steps

## Status 2026-10-29 (late)
Vast key is stored in the app and works, but the account has **no endpoint `comfyui-json`**. Next: Settings → «Δημιούργησε το endpoint αυτόματα» (untested; ask user first) or create it in the Vast console (template «Wan 2.2 14B t2v (Serverless)», creator 62897). Then follow the list below. On any failure: read the newest `report-*.txt` from `%APPDATA%\com.aivideostudio.app\logs` (Logs page → «Φτιάξε αναφορά»).

## First real run (user has keys now → do in this order)
1. Install from Desktop. Settings → Vast API key, endpoint `comfyui-json` → **Test Connection** (expect endpoint found; if "keeps workers hot" press *Apply scale-to-zero*).
2. Settings → provider (default Claude Code; Test Connection).
3. GPU & Cost → load prices → pick cards → Apply (watch Vast error text if `search_params` format is rejected).
4. Generate one clip with delivery = **cloud** first *only if* S3/R2 vars exist in Vast (verifies base flow), else go straight to local.
5. Models → *Install on workers* (adds node + models) → wait for new worker → Generate with delivery **local**.
6. If it fails, copy the job error from Queue → fix the matching assumption in [[Known limits]].

## Backlog
- Real cost calibration from finished jobs (avg duration per workflow/GPU).
- I2V workflow with image upload (`input` image URL is auto-downloaded by the wrapper per docs).
- Burn-in caption / metadata tag (needs ffmpeg or mp4 atom writer) if "prompt in video" meant that.
- Code signing for the installer; Greek NSIS language file.
- Streamed progress if wrapper exposes `/progress` (docs mention `progress_updates` only in the final reply).
- Auto-update, log viewer, per-project default mode.
