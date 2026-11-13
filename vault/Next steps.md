> **2026-11-13 (night):** v0.9.5 released (two RunPod instances). Next: (1) test instance switching live (UI and `set_defaults`), (2) Antonis builds his own volume/endpoint with the Desktop guide, or we do it with his temporary key, (3) live test of "Connect Claude and ZCode" and GLM in ZCode, (4) idle billing check, (5) translate new Settings strings.

> **2026-11-13 (end):** v0.9.4 released. Vast is gone; cloud = RunPod Serverless ([[RunPod]]). Next: (1) live test of the "Connect Claude and ZCode" button and of GLM in ZCode, (2) i2v and LTX/H3 through the app on RunPod, (3) confirm idle billing stays 0 (console balance vs ~$0.39 up to 14:00), (4) real update flow 0.9.x, (5) translate new inline Settings strings to the other languages.

> **2026-10-31 (end):** v0.7.8 released (GitHub, site, Desktop folder «Για τον Αντώνη»). Direct Vast card works (4 real Wan 2.2 i2v videos on RTX 4090), TLS + per-card token, ready signal, Settings «Connect to Claude» with .mcpb inside the installer. Next: (1) docker image with Wan models (measure image pull time first), (2) check the new Settings section visually and the update flow on Antonis' PC (he uses the .mcpb, extension 0.3.2), (3) optional «stop instead of delete» with cost warning / smaller disk, (4) real tests of the other packs on a card.

> **2026-10-30:** το app είναι πλέον δημόσιο (https://github.com/theslopvibe/ai-video-studio-for-windows, MIT, Ko-fi https://ko-fi.com/slopvibe, developer theslopvibe.com). Τρέχον πλάνο: redesign Image/Video/Audio + 10 γλώσσες, audio/music, editor (OpenCut/ffmpeg), auto-update από GitHub, διαχείριση PR άλλων, σελίδα Pages με screenshots. Λεπτομέρειες: `memory.md` (κορυφή) και `ROADMAP.md`.

---
tags: [ai-video-studio, todo]
---
# Next steps

See also: [[Research - Vast alternatives and face swap]] (replace Vast cards with serverless, add face swap; validation test first), [[Research - Beam and Modal]] (deep dive: Modal H100 first test, Beam only 4090 serverless).

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

- (2026-10-30) Re-run wizard, install packs one by one and fix workflows on first real runs (see Known limits 12). Then: SAM2 select-object tool, TTS/music language lists for ACE-Step, auto-update, issue/PR templates, screenshots page, deeper Models/GPU/Logs redesign, native review of translations, 29 EN/EL-only inline strings (+ new Chain/Editor strings are EN/EL only).

## 2026-10-30: v0.7.0 done
Signed auto-update (Settings > About > Check for updates), issue/PR templates. Next: test update flow with 0.7.1; real-hardware tests of tool packs; sha256 checks; screenshots page; Models/GPU/Logs redesign.

## 2026-11-01 (0.7.9-0.7.11)
- Δοκιμή σε πραγματική κάρτα: FLUX.2 klein pack/workflow, ρύθμιση «Μοντέλα στην κάρτα» (both / flux μόνο σε φθηνότερη GPU), t2v pack. Θέλει έγκριση κόστους.
- Να μην κλείνει η κάρτα όταν λείπει pack και τρέχει δουλειά (ή να μπαίνει pack σε τρέχουσα κάρτα).
- Επιβεβαίωση ενημέρωσης μέσα από την εφαρμογή (0.7.10 -> 0.7.11).
