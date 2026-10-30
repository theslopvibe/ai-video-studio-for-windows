# Changelog

## Unreleased

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

- Security hardening after an independent audit: no TLS fallback to workers, local API checks Host and compares the token in constant time (unauthenticated callers get an empty 404), file paths for images go through the same checks as other imports, editor bridge checks the origin, content security policy and a narrow asset scope for the window, node packs pinned to exact commits, imported model packs accept only .safetensors and .gguf, imported workflows cannot replace built-in modes.

- The local API for the Claude Desktop extension is now OFF by default (Settings > Claude Desktop connection). When off, nothing listens and no token file exists. File imports accept only local-drive paths (checked before touching the disk) and refuse links.

- Security: media inputs no longer put caller-chosen keys into file names, and file imports refuse network and device paths.
- Create video with chat: studio chats queue videos from confirm cards; ChatGPT (Codex CLI) and Gemini (Gemini CLI) subscriptions; Z.ai GLM Coding Plan.
- Video editor page: OpenCut (MIT) bundled and served locally, offline. Credits in About, README, `editor/NOTICE.md`.
- Text → Music: ACE-Step 3.5B (Apache-2.0) workflow and model pack (7.7 GB), Audio tab in Generate, audio playback in Library, chat can pick it.

## 0.3.0
- Full redesign: five places (Create, Chat, Video editor, Library, Settings) instead of eleven; Models, GPU & Cost, Projects, Guide and Logs are tabs inside Settings; the queue sits at the top of the Library. Create is one page with Video / Image / Music, the prompt first, and a sticky Generate bar. New look: ink and paper themes, amber "tally light" for the main action and for work in progress, Bahnschrift headings.
