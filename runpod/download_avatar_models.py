"""Downloads the InfiniteTalk 480p files onto the RunPod network volume (mounted at /workspace in a temporary pod).
Skips files that are already complete, writes to .part first, and downloads in parallel. Run once; safe to run again.
Files land in /workspace/ComfyUI/models/<folder>, which the serverless worker sees as /runpod-volume/ComfyUI/models/<folder>."""
import os, sys, threading, urllib.request

BASE = os.environ.get("AIVS_BASE", "/workspace/ComfyUI/models")
HF = "https://huggingface.co/"
FILES = [
    ("diffusion_models", HF + "Kijai/WanVideo_comfy_fp8_scaled/resolve/main/I2V/Wan2_1-I2V-14B-480p_fp8_e4m3fn_scaled_KJ.safetensors"),
    ("text_encoders", HF + "Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/main/split_files/text_encoders/umt5_xxl_fp8_e4m3fn_scaled.safetensors"),
    ("model_patches", HF + "Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/main/split_files/model_patches/wan2.1_infiniteTalk_single_fp16.safetensors"),
    ("audio_encoders", HF + "Kijai/wav2vec2_safetensors/resolve/main/wav2vec2-chinese-base_fp16.safetensors"),
    ("vae", HF + "Kijai/WanVideo_comfy/resolve/main/Wan2_1_VAE_bf16.safetensors"),
    ("loras", HF + "Kijai/WanVideo_comfy/resolve/main/Lightx2v/lightx2v_I2V_14B_480p_cfg_step_distill_rank64_bf16.safetensors"),
]
failed = []


def get(d, url):
    name = url.rsplit("/", 1)[1]
    os.makedirs(f"{BASE}/{d}", exist_ok=True)
    final, part = f"{BASE}/{d}/{name}", f"{BASE}/{d}/{name}.part"
    if os.path.exists(final) and os.path.getsize(final) > 1_000_000:
        print("have", name, os.path.getsize(final), flush=True)
        return
    try:
        done = os.path.getsize(part) if os.path.exists(part) else 0
        req = urllib.request.Request(url, headers={"Range": f"bytes={done}-"} if done else {})
        with urllib.request.urlopen(req, timeout=60) as r, open(part, "ab" if done else "wb") as f:
            total = done + int(r.headers.get("Content-Length", 0))
            n = done
            while True:
                b = r.read(8 << 20)
                if not b:
                    break
                f.write(b)
                n += len(b)
                if (n // (8 << 20)) % 64 == 0:
                    print(f"{name}: {n / 1e9:.1f}/{total / 1e9:.1f} GB", flush=True)
        if total and os.path.getsize(part) != total:
            raise RuntimeError(f"size {os.path.getsize(part)} != {total}")
        os.replace(part, final)
        print("done", name, os.path.getsize(final), flush=True)
    except Exception as e:
        print("FAILED", name, e, flush=True)
        failed.append(name)


ts = [threading.Thread(target=get, args=f) for f in FILES]
[t.start() for t in ts]
[t.join() for t in ts]
print("ALL DONE" if not failed else f"FAILED: {failed}", flush=True)
for d, u in FILES:
    p = f"{BASE}/{d}/{u.rsplit('/', 1)[1]}"
    print("size", p, os.path.getsize(p) if os.path.exists(p) else None, flush=True)
sys.exit(1 if failed else 0)
