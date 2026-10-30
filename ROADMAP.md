# Roadmap

## Done
1. **Setup wizard and local generation.** Hardware scan (NVIDIA, AMD, Intel), one-click ComfyUI install, models that fit the card, resumable downloads, per-video "Run on: this PC / cloud".
2. **More providers (first step).** Hosted APIs fal.ai and Replicate next to Vast.ai. Hosted models are JSON files, so new models need no code.
3. **Image generation.** FLUX.1 schnell and SDXL locally; FLUX on fal.ai and Replicate.
4. **Built-in chat.** Folders, search, pinning, per-chat provider and model. Providers: Claude (subscription or API), OpenAI GPT, Gemini, Z.ai GLM, DeepSeek, OpenRouter, Ollama.
5. **Release basics.** MIT license, README (English and Greek), contribution guide, CI, changelog.

## Next
- **Paid models next to the open-source ones (asked for, do this first).** Connect paid image and video models, for example OpenAI GPT Image, Google Gemini image, and other paid APIs. The user picks one, and the model list is split into two groups: Open source (runs on your PC or a rented GPU, free or cheap) and Paid (per result, needs an API key). Each model shows what it is good for (photorealism, text in images, editing a photo, fast drafts, long clips, character consistency, and so on) and a typical price, so a beginner can choose. The chat and the Claude Desktop tools should be able to recommend a model for a task.
- **Test on clean machines.** The full local install has been run on one PC (RTX 4060, 8 GB). Needs AMD, Intel and low-disk cases.
- **More providers.** RunPod serverless, Modal, Together; provider-specific price lists in the wizard.
- **Chat.** Streaming replies, Markdown and code blocks, attachments, import and export of conversations, "turn this chat into a storyboard and queue the clips".
- **Editing tools.** Trim, join clips of a story, add music and captions.
- **Polish.** Interface pass for a premium look, in-app updates, code signing, a website, more languages.
- **Vast.ai image modes.** Images through the Vast worker (videos work today).

## Next: editor, audio and redesign (requested 2026-10-30)
1. **Audio / music / sound-effects generator.** Local first through the existing ComfyUI install (Stable Audio Open, ACE-Step for music, MMAudio for video-to-sound), same workflow-JSON and model-pack mechanism as video. Hosted modes as paid options with `license: paid`, `good_for`, `est_usd`.
2. **Video editor connected to the library.** (a) built-in simple editor (trim, order clips, text, music track, export with ffmpeg), (b) "Open in Shotcut / Kdenlive / OpenShot" for full editing, passing the project's clips. GPL tools are launched, not bundled.
3. **UI redesign.** More practical and more beautiful: one workspace (Create / Library / Edit / Audio), model picker with Open source vs Paid and "good for" tips, per-project timeline, more options per mode.
4. **Ten languages.** The 10 most spoken (total speakers): English, Mandarin Chinese, Hindi, Spanish, French, Arabic, Bengali, Portuguese, Russian, Urdu. Greek stays as a bonus. Needs right-to-left layout for Arabic and Urdu, a language picker, and the strings split out of the components (today only EN/EL, in `i18n.ts`, `chatStrings.ts`, `guide.ts`, `wizard.ts`, workflow `name_*`/`desc_*`).
5. **Donate service.** Create the account (Ko-fi, Buy Me a Coffee or GitHub Sponsors), then set `donate.url` in `slopvibe-site/data/site.json`. The app already links to the site page.
