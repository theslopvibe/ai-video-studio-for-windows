# Changelog

## 0.9.7

- **Vast.ai is back, next to RunPod.** Settings has "Which cloud do you use?": Vast.ai (rented cards) or RunPod. Only the cloud you pick works: its panel, tools and modes are shown and used, the other one stays idle and hidden (its keys stay saved). This covers videos from the app and from Claude / GLM (the other cloud's MCP tools answer with a clear message). Existing installs keep RunPod if a RunPod key is saved, otherwise Vast if a Vast key is saved. The Vast side is the one from 0.8.1 (cards, GPU and cost tab, speed test, watchdog, several cards). MCP package 0.6.0 (the `.mcpb` has the Vast tools again).

## 0.9.6

- **Video to video.** New mode "Video → New video from your clip (Wan 2.2 restyle)" in the Video tools: give a clip and a prompt, and it changes the look while keeping the motion (a Change slider: 1 big, 3 subtle). Runs on your own PC with a strong GPU; not available on RunPod yet (the RunPod worker cannot take a video input). Untested on a real ComfyUI so far.

## 0.9.5

- **Two RunPod instances.** Settings > RunPod has an "instance in use" picker with a name for each (for example "Mine" and "Antonis"), each with its own API key and endpoint id, so two different accounts with their own billing can live in one app. Only the selected one runs jobs, wakes and sleeps; the other is put to sleep too when idle or when the app closes. Claude and GLM can switch with `set_defaults` (`runpod_instance`) and see the one in use in `runpod_status`. Everything an instance holds (models, workflows) is up to its owner: add your own workflows with Import in the Models page. MCP package 0.5.0.
- `docs/runpod/setup_models.py` downloads the models onto your own RunPod network volume (resumable), for people who build their own instance.

## 0.9.4

- A RunPod spend limit value that cannot be read now falls back to 5 USD instead of "no limit". Empty still means no limit.

## 0.9.3

- The RunPod spend limit is off by default (0). Set one in Settings if you want it. When a limit is set, the cost of a job is booked before it starts, so parallel jobs cannot pass it together.

## 0.9.2

- **One button connects Claude Desktop and ZCode (GLM).** Settings > Claude connection > "Connect Claude and ZCode" adds the app to both (your other settings are kept, a backup is made). Close and reopen them once. GLM then works like Claude: it can wake RunPod, start videos and pictures and chain tools. Manual options and `Connect-ZCode.bat` stay as a fallback.
- **RunPod spend limit.** After $5 spent in one app session (change it in Settings > RunPod spend limit, 0 = no limit) no more RunPod jobs start. It can only be changed inside the app, never from Claude or ZCode.
- Your RunPod key stays inside the app (Windows Credential Manager). It is never sent to Claude, ZCode or the MCP connection, never shown in settings, and is removed from logs and reports.

## 0.9.1

- **GLM works like Claude through ZCode.** New Settings step "2c - ZCode (GLM)" and `Connect-ZCode.bat`: one double-click adds the app to ZCode (`~/.zcode/cli/config.json`), so GLM can wake RunPod, start videos and pictures, and chain tools with the same confirmation and price rules as Claude. Your other ZCode settings are kept (a backup is made).

## 0.9.0

- **Cloud GPU is now RunPod Serverless instead of Vast.ai.** No more renting and waiting for a card: you pay only for the seconds a video or picture takes (about $0.02 for a FLUX picture, $0.06 for a 5 s Wan video, measured on an A40). The models live on a RunPod network volume, so nothing is downloaded per video. Enter your RunPod key and endpoint id in Settings; the old Vast cards, GPU page and card settings are gone.
- **Wake up / sleep.** The endpoint sleeps by itself (workers ceiling 0) so nothing can bill while you are away. It wakes when a job is sent, or with the "Wake up RunPod" button, and goes back to sleep after the idle time you choose or when the app closes. A chip shows the state and this session's spend.
- **Five RunPod workflows:** FLUX.3 klein pictures, Wan 2.2 text → video and picture → video (lightning), LTX-2.5, MiniMax H3.
- **Claude chat:** GLM default is now glm-5.1 (glm-5, glm-4.6, glm-4.5-air also listed; the live list from Z.ai is added). New MCP tools `runpod_status`, `runpod_wake`, `runpod_sleep` (MCP package 0.4.0).
- Tested with a real endpoint: wake, one FLUX picture, sleep (about 85 s, $0.019).

## 0.8.1

- **Two new video models to try: LTX-2.5 and MiniMax H3** (text → video and picture → video, with sound). Both are big (about 40 GB each) and need a card with at least 32-48 GB. They are new and not measured yet, so speed and quality may change. Install them on the Models page or ask Claude to start a card with them. MiniMax H3 has its own community licence: read it before using videos commercially. LTX-2.5 files come from an unofficial mirror of the Lightricks release.

## 0.8.0

- **Several cloud cards at the same time.** Videos now run in parallel on separate cards, up to "Cards at the same time" (Models page, 1-4, default 2). Before, everything waited in one line on one card. A card in use is never given to a second job; an idle card with the right models is reused; extra cards are rented only when needed.
- **Pictures-only card.** Text → Image jobs get their own cheap FLUX-only card (about 12 GB to download instead of about 50 GB), so pictures start much faster. Turn it off with "Pictures" on the Models page. New buttons: "Start another card (video)" and "Start a pictures-only card".
- **Cards are checked every minute.** The app closes a card that Vast stopped, that stopped answering for 3 minutes, that was never ready after 45 minutes, or that is idle for the auto-close time, and shows a notification. A dead card no longer bills for hours. During the first download the log is checked every 8-30 seconds (was up to 2 minutes).
- **Everything works from Claude chat.** `card_start` has `kind` ("video"/"image") and `new_card` (a second card in parallel); `set_defaults` sets `card_max`, `image_card`, `auto_close_min`; `vast_running` shows each card with its own `ready`, `in_use` and `kind`. MCP package 0.3.3.

## 0.7.15

- Every new cloud card now runs a speed test first (200 MB from the model server). A card that downloads slower than about 400 Mbit/s is closed within a minute and the next one is rented, instead of waiting 40 minutes. The advertised "1 Gbit/s" of a machine is often not what you really get.
- Slow machines (and machines that cannot pull the image) are remembered for 7 days and skipped in later searches.
- Up to 8 machines are tried before giving up (was 4). The log shows the measured speed of the model server and of a neutral server, so we can see who is slow.

## 0.7.14

- A Windows notification appears when the cloud card is ready and its models are downloaded.
- The app now checks that the card really has the model files of the job (ComfyUI lists them) before sending work. A card without them is replaced (old card) or closed with a clear message (new card whose download failed), instead of failing every job.
- Downloads that failed in the first round are tried again one by one on the card.
- Vast "429 Too Many Requests": the app waits for the time Vast asks and tries again, so many jobs sent at once no longer fail.

## 0.7.13

- New: Text → Image with FLUX.3 klein (int8, 4-16 steps, about 11 GB pack, FLUX Kommunity License). The "FLUX" in "Models on the card" now means FLUX.3 klein, so Wan + FLUX.3 sit on the same card. If you have your own workflow with the id flux3-klein-t2i, the app shows a message: delete it, the built-in one has the model pack.

## 0.7.12

- "Models on the card" now starts as "Wan + FLUX together": one card holds both, so images and clips work without changing anything. Pick another option on the Models page to save money.

## 0.7.11

- New choice "Models on the card" (Models page): only what each job needs, Wan + FLUX together on one card, Wan only, or FLUX only. With Wan the strong card is used (RTX 4090 by default); with FLUX only the cheapest card of 16 GB or more is rented (RTX 3090 was about 0.17 USD/h).
- Cards are rented only on machines with a 1 Gbit/s or faster download link (the 400 Mbit/s fallback is gone).

## 0.7.10

- New: Text → Image with FLUX.2 klein 4B (4 steps, 12 GB card). Small model pack (about 12 GB, fp8) so the rented card starts fast and costs little. Apache-2.0.

## 0.7.9

- Fixed: Wan 2.2 text-to-video (Fast and Quality) failed instantly with "This mode has no model pack to install on a card". Both modes now know their model pack, so the Vast card can install it.

## 0.7.8

- The connection to the rented card is now encrypted (TLS with a certificate made for each card and trusted only by the app), the token is compared in constant time, and every new card gets a new token and certificate.
- Claude chat can tell when the card is ready: `vast_running` returns `ready` (true only when ComfyUI on the card accepts work) and `progress`. The app also signals ready. Extension 0.3.2.
- Faster card setup: only machines with a 1 Gbit/s or faster link are rented first, and the model files download side by side.
- New "Connect to Claude" section in Settings, with the extension file (.mcpb) and Connect-Claude.bat shipped inside the installer: what it does, how it works, steps for Claude Desktop and a copy-paste command for Claude Code.

## 0.7.7

- Vast now runs on a real rented card instead of serverless (serverless never produced a ready worker). The first video rents the GPU you picked (for example RTX 4090), installs the model on it and runs ComfyUI behind a private token; the following videos reuse the same card. First real Wan 2.2 image-to-video made this way on an RTX 4090.
- The card is closed by the Close button, by the idle timer (default 10 minutes) or from Claude chat (vast_stop). A machine that cannot pull the image is replaced automatically (up to 4 tries); machines in China are skipped.
- Claude chat: "make a video with Wan 2.2 from this picture on a 4090" is understood (generic words like "image to video" are ignored when matching the model). card_start accepts workflow_id and gpu to prepare a specific card in advance.

## 0.7.6

- Claude Desktop connector built into the program (`app.exe --mcp`), for computers where the .mcpb extension is not accepted. Run mcp/Connect-Claude.bat once; it adds the connector to claude_desktop_config.json (backup kept). No extension file and no Node.js needed.

## 0.7.5

- Models page: one simple "My card (Vast)" box with the status (off / starting / ready), Start card, Delete card, the favourite card and an automatic close timer (default 10 minutes idle, so a forgotten card never keeps billing).
- Claude Desktop extension 0.3.0: card_start, vast_running, vast_stop. Closing a card also cancels waiting jobs so Vast does not open a new one. Closing only touches the app's own cards and "close all" must be explicit.

## 0.7.4

- Models page: new panel "What is running on Vast right now" shows every rented card (GPU, status, minutes, cost so far, $/hour) with a Close button, a warning for a card stuck in loading over 10 minutes, and refreshes every 10 seconds. Claude Desktop extension 0.2.2 can ask what is running (vast_running) and close cards (vast_stop).
- Fix: after "Install on workers" the card choice (for example RTX 4090) is applied again on the next job or "Prepare now" (linking the template had reset it).

## 0.7.3

- New button "Prepare now" on the Models page: wakes a cloud card on Vast right away so the models download in advance, shows progress, and tells you when it is ready so the first video starts at once.

## 0.7.2

- Fix: "Install on workers" failed on Vast with "template search_query must be a dict, got str". The template filters are now sent as an object.

## 0.7.1

- Fix: "Install on workers" and the GPU choice failed on Vast with "Missing endpoint ID". Vast needs the endpoint id on workergroup updates; the app now sends it.

## 0.7.0

- Automatic updates: Settings > About > "Check for updates" downloads and installs signed new versions and keeps your data. Install this version by hand once; later versions update from inside the app.

- The Claude Desktop connection switch now works immediately (no app restart). The extension gives clear step-by-step messages in Greek when the app is closed or the connection is off.

- New tools: subtitles from speech (Whisper, 99 languages, .srt/.vtt, shown in the Library), cut out or erase an object by clicking on it (SAM2 + LaMa), restore faces (GFPGAN), colorize black and white photos (DDColor), depth map (Depth Anything 3), picture to 3D model (TRELLIS.2, .glb). The image editor's AI menu has the picture tools. All are untested on real hardware yet.
- Packs can install pinned pip packages (`pip`, `pip_nodeps`), and the helper node file gained speech-to-subtitles, LaMa erase, face restore and point helpers.

- About page: removed the personal paragraph about the developer.

- Step-by-step Greek guide for using Vast and the Claude Desktop extension: mcp/GUIDE.el.html.

- Security hardening after an independent audit: no TLS fallback to workers, local API checks Host and compares the token in constant time (unauthenticated callers get an empty 404), file paths for images go through the same checks as other imports, editor bridge checks the origin, content security policy and a narrow asset scope for the window, node packs pinned to exact commits, imported model packs accept only .safetensors and .gguf, imported workflows cannot replace built-in modes.

- The local API for the Claude Desktop extension is now OFF by default (Settings > Claude Desktop connection). When off, nothing listens and no token file exists. File imports accept only local-drive paths (checked before touching the disk) and refuse links.

- Security: media inputs no longer put caller-chosen keys into file names, and file imports refuse network and device paths.

- Everything chains: "Make more from this…" in the Library turns any result into the next step (picture to video, video to smoother motion, voice to lip-sync). The Create page has two new tabs, Voice and Tools. Chat and the Claude Desktop extension can run the same chains (`job:<id>` references, `inputs`).
- Voice: Chatterbox (23 languages incl. Greek, voice cloning from your own sample, voice changer, MIT) and Kokoro (fast, 9 languages, CPU). Each mode lists its languages.
- Picture tools: background removal (BiRefNet), 4x upscale (Real-ESRGAN), edit with words (Qwen-Image-Edit). Also in the image editor's AI menu.
- Video tools: smoother motion (RIFE), lip-sync (LatentSync, experimental, needs ~18 GB video memory).
- Not included on purpose: face swap (the usual InsightFace models forbid commercial use) and other non-commercial models.
- Model packs can install custom nodes and keep weights next to them; modes can declare drop-down choices, languages and a safety notice.

- Image editor (miniPaint, MIT) next to the video editor, with an AI menu: generate an image into a layer.

- Setup wizard: hardware scan, one-click local ComfyUI install, models that fit your graphics card, first video.
- Local generation on NVIDIA (AMD and Intel experimental), with resumable downloads that survive a stalled connection.
- Hosted providers: fal.ai and Replicate (pay per result, no GPU). Models are editable JSON files.
- Image generation: FLUX.1 schnell and SDXL, locally and hosted.
- Built-in chat with folders, search, pinning and several providers: Claude (subscription or API), OpenAI GPT, Gemini, Z.ai GLM, DeepSeek, OpenRouter, Ollama.
- Claude Desktop extension (MCP) to generate videos from chat, with a local API.
- English first interface and messages, Greek as the second language.
- MIT license, README, roadmap and contribution guide.

## 0.1.0

- First version: Vast.ai Serverless with Wan 2.2, queue, library, projects, image to video, models from Hugging Face, logs and reports.

## Unreleased

- The Claude Desktop connection switch now works immediately (no app restart). The extension gives clear step-by-step messages in Greek when the app is closed or the connection is off.

- Security hardening after an independent audit: no TLS fallback to workers, local API checks Host and compares the token in constant time (unauthenticated callers get an empty 404), file paths for images go through the same checks as other imports, editor bridge checks the origin, content security policy and a narrow asset scope for the window, node packs pinned to exact commits, imported model packs accept only .safetensors and .gguf, imported workflows cannot replace built-in modes.

- The local API for the Claude Desktop extension is now OFF by default (Settings > Claude Desktop connection). When off, nothing listens and no token file exists. File imports accept only local-drive paths (checked before touching the disk) and refuse links.

- Security: media inputs no longer put caller-chosen keys into file names, and file imports refuse network and device paths.
- Create video with chat: studio chats queue videos from confirm cards; ChatGPT (Codex CLI) and Gemini (Gemini CLI) subscriptions; Z.ai GLM Coding Plan.
- Video editor page: OpenCut (MIT) bundled and served locally, offline. Credits in About, README, `editor/NOTICE.md`.
- Text → Music: ACE-Step 3.5B (Apache-2.0) workflow and model pack (7.7 GB), Audio tab in Generate, audio playback in Library, chat can pick it.

## 0.3.0
- Full redesign: five places (Create, Chat, Video editor, Library, Settings) instead of eleven; Models, GPU & Cost, Projects, Guide and Logs are tabs inside Settings; the queue sits at the top of the Library. Create is one page with Video / Image / Music, the prompt first, and a sticky Generate bar. New look: ink and paper themes, amber "tally light" for the main action and for work in progress, Bahnschrift headings.
