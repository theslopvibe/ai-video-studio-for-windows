# AI Video Studio For Windows

A free, open-source Windows desktop app that turns a text prompt or a picture into a video or an image with open-source AI models (LTX-Video, Wan 2.2, and more). Think of it as a desktop alternative to Higgsfield and similar web tools, except it runs on your own graphics card or on a cloud GPU you rent by the minute, and everything you make stays on your computer.

Developed by [theslopvibe.com](https://theslopvibe.com). Released under the MIT license.

Ελληνικά: [README.el.md](README.el.md)

## What it does today

- **Setup wizard.** Detects your graphics card, installs ComfyUI for you, picks models that fit your card, and makes a first video. No command line.
- **Runs on this PC or in the cloud.** NVIDIA is the safest choice; AMD and Intel are supported experimentally. Cloud: Vast.ai Serverless (rent a GPU) or fal.ai and Replicate (pay per result).
- **Text to video, image to video, start and end frame.** Wan 2.2 and LTX-Video workflows are bundled. Any ComfyUI workflow you export can be dropped in.
- **Queue, library and projects.** Batch many prompts, keep clips in project folders, see the cost of each clip.
- **Add models from Hugging Face.** Search or paste a link; the app works out the type, VRAM need and license.
- **Image generation.** FLUX.1 schnell and SDXL, on your PC or hosted.
- **Hosted providers.** fal.ai and Replicate: pay per result, no GPU and no setup. Models are editable JSON files.
- **Built-in chat** with folders, search and pinning. Use your Claude subscription or API keys for Claude, OpenAI GPT, Gemini, Z.ai GLM, DeepSeek, OpenRouter, or run Ollama locally. Send any answer to the generator as a prompt.
- **Use it from Claude Desktop.** A bundled MCP extension lets Claude generate videos for you from chat (`mcp/`).
- **Private by default.** API keys live in the Windows Credential Manager. Logs hide secrets.

See [ROADMAP.md](ROADMAP.md) for what is next.

## Install

☕ Free forever. If it helps you, [buy us a coffee on Ko-fi](https://ko-fi.com/slopvibe).

1. Download the latest `AI Video Studio For Windows_<version>_x64-setup.exe` from the Releases page.
2. Run it. It installs for the current user and needs no admin rights. If Windows shows "Windows protected your PC", choose **More info → Run anyway** (the installer is not code-signed yet).
3. Follow the setup wizard.

Requirements: Windows 10 or 11. For local generation, an NVIDIA card with 8 GB of VRAM or more, and about 20 GB of free disk space. Without a strong card, use a cloud GPU.

## Build from source

```
cd app
npm install
npm run tauri build          # installer in app/src-tauri/target/release/bundle/nsis
```

Needs Node 20+, Rust (stable) and the Tauri 2 prerequisites for Windows.

Work on the interface without the Rust backend: `npm run dev`, then open `http://localhost:1420/?mock`.

Tests: `cd app/src-tauri && cargo test`, and `npx svelte-check` in `app/`. End-to-end tests against a mock Vast server are in `app/tests/e2e`.

## Project layout

| Path | What |
|---|---|
| `app/src` | Svelte 5 interface (English first, Greek second) |
| `app/src-tauri/src` | Rust backend: job queue, providers (`vast.rs`, `local.rs`), local API for Claude (`api.rs`) |
| `app/src-tauri/workflows` | Bundled ComfyUI workflows |
| `mcp/` | Claude Desktop extension (MCP server) |
| `vault/` | Longer design notes |

## Contributing

Issues and pull requests are welcome. Please keep the interface text in both languages (`app/src/lib/i18n.ts` and `wizard.ts`), run the tests, and describe what you changed and why.

## License

MIT, see [LICENSE](LICENSE). The AI models you download have their own licenses; the app shows each model's license before you install it.

## Video editor

The **Video editor** page is [OpenCut](https://github.com/opencut-app/opencut-classic) (open source, MIT license, by the OpenCut contributors), built into the app and running fully offline: timeline, multiple tracks, audio, text, effects, export. It is not our work; we only change how it is built (see [`editor/NOTICE.md`](editor/NOTICE.md)). Build it with `python editor/build.py`.
