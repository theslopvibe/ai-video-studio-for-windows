---
tags: [ai-video-studio, runpod]
---
# RunPod (since 0.9.0)

- Endpoint `n1q8r5vkmex43u` (`aivs-comfy-a40`, QUEUE, A40 48GB, 1.22 USD/h per second, FlashBoot). Start command points ComfyUI at the volume and updates ComfyUI (needed for CLIPLoader `flux3`).
- Network volume `aivs-models` `cvswh9ahlu`, 200 GB STANDARD EU-SE-1 (~14 USD/month), holds FLUX.3 klein, Wan 2.2, LTX-2.5, MiniMax H3 under `/runpod-volume/ComfyUI/models/`.
- Asleep = workers.max 0. Wake = PATCH `https://api.runpod.io/v2/serverless/{id}` workers.max 1; runtime says 409 ENDPOINT_PAUSED for 10-30 s after. Jobs go to `https://api.runpod.ai/v2/{id}/run`; output returns as base64 in `output.images[]`.
- Measured on A40: FLUX 48 s (~$0.016), Wan 5 s 184 s (~$0.063), LTX 188 s (~$0.064), H3 351 s (~$0.119). Cold start 30-70 s.
- App code: `runpod.rs` (wake/sleep/auto-sleep/spend), `hosted.rs run_runpod`, workflows `runpod-*.json`, UI `RunpodChip`/`RunpodPanel`. Real test: `cargo test real_runpod -- --ignored --nocapture` (spends ~$0.02).
- Claude/ZCode connection: Settings button `connect_clients` (lib.rs). Keys: Credential Manager only.
- See [[Decisions]], [[Next steps]], [[Research - Vast alternatives and face swap]].
