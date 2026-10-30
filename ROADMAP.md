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
