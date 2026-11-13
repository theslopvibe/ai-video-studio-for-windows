#!/usr/bin/env python3
"""Downloads the AI Video Studio models onto a RunPod network volume.

Run it in the web terminal of a temporary Pod that has your network volume mounted at /workspace:
    python3 setup_models.py                 # everything (about 150 GB)
    python3 setup_models.py flux3-klein wan22-t2v wan22-i2v     # only these (about 90 GB)
Files go to /workspace/ComfyUI/models/<folder>, which the serverless endpoint sees as /runpod-volume/ComfyUI/models/<folder>.
It skips files that are already complete and resumes broken downloads, so you can run it again.
Some mirrors need a Hugging Face token: export HF_TOKEN=hf_xxx first (huggingface.co/settings/tokens, read access).
"""
import os, sys, urllib.request, urllib.error

BASE = os.environ.get("AIVS_BASE", "/workspace/ComfyUI/models")
PACKS = {
 "wan22-t2v": {
  "size_gb": 38,
  "files": [
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/diffusion_models/wan2.2_t2v_high_noise_14B_fp8_scaled.safetensors"
   },
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/diffusion_models/wan2.2_t2v_low_noise_14B_fp8_scaled.safetensors"
   },
   {
    "dir": "text_encoders",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/text_encoders/umt5_xxl_fp8_e4m3fn_scaled.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/vae/wan_2.1_vae.safetensors"
   },
   {
    "dir": "loras",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/loras/wan2.2_t2v_lightx2v_4steps_lora_v1.1_high_noise.safetensors"
   },
   {
    "dir": "loras",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/loras/wan2.2_t2v_lightx2v_4steps_lora_v1.1_low_noise.safetensors"
   }
  ]
 },
 "wan22-i2v": {
  "size_gb": 38,
  "files": [
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/diffusion_models/wan2.2_i2v_high_noise_14B_fp8_scaled.safetensors"
   },
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/diffusion_models/wan2.2_i2v_low_noise_14B_fp8_scaled.safetensors"
   },
   {
    "dir": "text_encoders",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/text_encoders/umt5_xxl_fp8_e4m3fn_scaled.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/vae/wan_2.1_vae.safetensors"
   },
   {
    "dir": "loras",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/loras/wan2.2_i2v_lightx2v_4steps_lora_v1_high_noise.safetensors"
   },
   {
    "dir": "loras",
    "url": "https://huggingface.co/Comfy-Org/Wan_2.2_ComfyUI_Repackaged/resolve/main/split_files/loras/wan2.2_i2v_lightx2v_4steps_lora_v1_low_noise.safetensors"
   }
  ]
 },
 "flux3-klein": {
  "size_gb": 11,
  "files": [
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/ml-resources/flux-3-image-klein-comfy/resolve/main/diffusion_models/flux3-image-klein-int8-convrot.safetensors"
   },
   {
    "dir": "text_encoders",
    "url": "https://huggingface.co/ml-resources/flux-3-image-klein-comfy/resolve/main/text_encoders/qwen3vl_4b_int8_convrot.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/ml-resources/flux-3-image-klein-comfy/resolve/main/vae/flux2-vae.safetensors"
   }
  ]
 },
 "ltx25-22b": {
  "size_gb": 39.7,
  "files": [
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/comfyicu/LTX-2.5/resolve/main/diffusion_models/ltx-2.5-22b-distilled-transformer-comfy-int8-convrot.safetensors"
   },
   {
    "dir": "text_encoders",
    "url": "https://huggingface.co/comfyicu/LTX-2.5/resolve/main/text_encoders/gemma4-12b-with-proj-ltx-2.5-comfy-int8-convrot.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/comfyicu/LTX-2.5/resolve/main/vae/ltx-2.5-video-vae-bf16.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/comfyicu/LTX-2.5/resolve/main/vae/ltx-2.5-audio-vae-bf16.safetensors"
   },
   {
    "dir": "latent_upscale_models",
    "url": "https://huggingface.co/comfyicu/LTX-2.5/resolve/main/latent_upscale_models/ltx-2.5-latent-spatial-upscaler-x2-bf16-1.0.safetensors"
   }
  ]
 },
 "minimax-h3": {
  "size_gb": 42.5,
  "files": [
   {
    "dir": "diffusion_models",
    "url": "https://huggingface.co/Comfy-Org/MiniMax-H3/resolve/main/diffusion_models/minimax_h3_fl2va_pruned_int8_convrot.safetensors"
   },
   {
    "dir": "text_encoders",
    "url": "https://huggingface.co/Comfy-Org/MiniMax-H3/resolve/main/text_encoders/qwen3vl_32b_minimax_h3_nvfp4_awq.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/Comfy-Org/MiniMax-H3/resolve/main/vae/minimax_h3_video_vae_fp16.safetensors"
   },
   {
    "dir": "vae",
    "url": "https://huggingface.co/Comfy-Org/MiniMax-H3/resolve/main/vae/minimax_h3_audio_vae_fp32.safetensors"
   }
  ]
 }
}

def size_of(url, headers):
    req = urllib.request.Request(url, method="HEAD", headers=headers)
    with urllib.request.urlopen(req, timeout=60) as r:
        return int(r.headers.get("Content-Length", "0"))

def fetch(url, dest):
    headers = {"User-Agent": "aivs-setup"}
    tok = os.environ.get("HF_TOKEN")
    if tok and "huggingface.co" in url:
        headers["Authorization"] = "Bearer " + tok
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    try:
        total = size_of(url, headers)
    except Exception as e:
        total = 0
    have = os.path.getsize(dest) if os.path.exists(dest) else 0
    if total and have == total:
        print("  ok (already there):", os.path.basename(dest)); return
    for attempt in range(12):
        have = os.path.getsize(dest) if os.path.exists(dest) else 0
        h = dict(headers)
        if have:
            h["Range"] = "bytes=%d-" % have
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=h), timeout=60) as r, open(dest, "ab" if have else "wb") as f:
                while True:
                    b = r.read(1 << 20)
                    if not b: break
                    f.write(b)
            if not total or os.path.getsize(dest) >= total:
                print("  done:", os.path.basename(dest), round(os.path.getsize(dest) / 1e9, 2), "GB"); return
        except urllib.error.HTTPError as e:
            if e.code == 416: print("  done:", os.path.basename(dest)); return
            if e.code in (401, 403): sys.exit("  %s: access denied (%d). Set HF_TOKEN and accept the model licence on its Hugging Face page." % (url, e.code))
            print("  retry", attempt + 1, e)
        except Exception as e:
            print("  retry", attempt + 1, e)
    sys.exit("failed: " + url)

def main():
    ids = sys.argv[1:] or list(PACKS)
    for i in ids:
        if i not in PACKS: sys.exit("unknown pack %s (choose from %s)" % (i, ", ".join(PACKS)))
    print("Total about", sum(PACKS[i]["size_gb"] for i in ids), "GB")
    for i in ids:
        print("==", i)
        for f in PACKS[i]["files"]:
            fetch(f["url"], os.path.join(BASE, f["dir"], f["url"].split("/")[-1].split("?")[0]))
    print("All done. Terminate the temporary Pod now (the volume keeps the files).")

main()
