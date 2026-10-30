# AI Video Studio: returns the finished video inline (base64) so it can be saved on the user's PC
# without any S3/R2 bucket. Installed into ComfyUI/custom_nodes/aivs_save_b64/__init__.py.
import base64
import io
import os
import tempfile

import numpy as np
import torch
from PIL import Image


class AIVSSaveVideoBase64:
    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {"video": ("VIDEO",)}}

    RETURN_TYPES = ()
    FUNCTION = "run"
    OUTPUT_NODE = True
    CATEGORY = "AIVS"

    def run(self, video):
        fd, path = tempfile.mkstemp(suffix=".mp4")
        os.close(fd)
        try:
            video.save_to(path)
            with open(path, "rb") as f:
                data = base64.b64encode(f.read()).decode("ascii")
        finally:
            try:
                os.remove(path)
            except OSError:
                pass
        return {"ui": {"aivs_video_b64": [data]}}


class AIVSLoadImageBase64:
    """Image-to-video input: the start image travels inside the workflow as base64 (same outputs as LoadImage)."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {"image_b64": ("STRING", {"multiline": True, "default": ""})}}

    RETURN_TYPES = ("IMAGE", "MASK")
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, image_b64):
        img = Image.open(io.BytesIO(base64.b64decode(image_b64))).convert("RGB")
        arr = np.asarray(img).astype(np.float32) / 255.0
        image = torch.from_numpy(arr)[None,]
        mask = torch.zeros((1, arr.shape[0], arr.shape[1]), dtype=torch.float32)
        return (image, mask)


def _temp_file(b64, suffix):
    fd, path = tempfile.mkstemp(suffix=suffix)
    with os.fdopen(fd, "wb") as f:
        f.write(base64.b64decode(b64))
    return path


class AIVSLoadAudioBase64:
    """Voice / lip-sync input: the sound file travels inside the workflow as base64 (same output as LoadAudio)."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {"audio_b64": ("STRING", {"multiline": True, "default": ""})}}

    RETURN_TYPES = ("AUDIO",)
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, audio_b64):
        import av

        path = _temp_file(audio_b64, ".audio")
        try:
            with av.open(path) as container:
                stream = container.streams.audio[0]
                rate = stream.codec_context.sample_rate
                chunks = []
                for frame in container.decode(stream):
                    chunks.append(torch.from_numpy(frame.to_ndarray()).float())
            layout = chunks[0].shape[0] if chunks[0].dim() > 1 else 1
            if layout == 1 and stream.codec_context.layout.nb_channels > 1:
                # packed (interleaved) samples: [1, C*T] -> [C, T]
                ch = stream.codec_context.layout.nb_channels
                wave = torch.cat(chunks, dim=1).reshape(-1, ch).T
            else:
                wave = torch.cat(chunks, dim=1)
            if wave.abs().max() > 1.5:
                wave = wave / 32768.0
        finally:
            try:
                os.remove(path)
            except OSError:
                pass
        return ({"waveform": wave.unsqueeze(0).contiguous(), "sample_rate": rate},)


class AIVSLoadVideoBase64:
    """Lip-sync / interpolation input: the clip travels inside the workflow as base64 (same output as LoadVideo)."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {"video_b64": ("STRING", {"multiline": True, "default": ""})}}

    RETURN_TYPES = ("VIDEO",)
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, video_b64):
        from comfy_api.input_impl import VideoFromFile

        path = _temp_file(video_b64, ".mp4")
        return (VideoFromFile(path),)



def _srt_time(t, vtt):
    ms = int(round(t * 1000))
    h, ms = divmod(ms, 3600000)
    m, ms = divmod(ms, 60000)
    sec, ms = divmod(ms, 1000)
    return f"{h:02d}:{m:02d}:{sec:02d}{'.' if vtt else ','}{ms:03d}"


class AIVSTranscribeSubtitles:
    """Speech to subtitles (faster-whisper, MIT). Writes an .srt/.vtt file to the output folder and reports it under "subtitles"."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {
            "audio": ("AUDIO",),
            "model": (["turbo", "large-v3", "medium", "small"],),
            "language": ("STRING", {"default": "auto"}),
            "task": (["transcribe", "translate"],),
            "format": (["srt", "vtt"],),
            "prefix": ("STRING", {"default": "subtitles"}),
        }}

    RETURN_TYPES = ("STRING", "STRING")
    RETURN_NAMES = ("subtitle_text", "language")
    FUNCTION = "run"
    CATEGORY = "AIVS"
    OUTPUT_NODE = True

    def run(self, audio, model, language, task, format, prefix):
        import gc

        import folder_paths
        import torchaudio
        from faster_whisper import WhisperModel

        if audio is None:
            raise RuntimeError("This video has no sound track, so there is nothing to transcribe.")
        wave = audio["waveform"][0].float().mean(0)
        wave = torchaudio.functional.resample(wave, int(audio["sample_rate"]), 16000).cpu().numpy().astype(np.float32)
        root = os.path.join(folder_paths.models_dir, "faster-whisper")
        os.makedirs(root, exist_ok=True)
        device = "cuda" if torch.cuda.is_available() else "cpu"
        wm = WhisperModel(model, device=device, compute_type="int8_float16" if device == "cuda" else "int8", download_root=root)
        try:
            segs, info = wm.transcribe(wave, language=None if language in ("", "auto") else language, task=task, vad_filter=True)
            segs = [(s.start, s.end, s.text.strip()) for s in segs]
        finally:
            del wm
            gc.collect()
            if device == "cuda":
                torch.cuda.empty_cache()
        vtt = format == "vtt"
        body = ("WEBVTT\n\n" if vtt else "") + "\n".join(f"{i}\n{_srt_time(a, vtt)} --> {_srt_time(b, vtt)}\n{t}\n" for i, (a, b, t) in enumerate(segs, 1))
        out = folder_paths.get_output_directory()
        prefix = "".join(c for c in prefix if c.isalnum() or c in "-_") or "subtitles"
        n = 1
        while os.path.exists(os.path.join(out, f"{prefix}_{n:05d}.{format}")):
            n += 1
        name = f"{prefix}_{n:05d}.{format}"
        with open(os.path.join(out, name), "w", encoding="utf-8") as f:
            f.write(body)
        return {"ui": {"subtitles": [{"filename": name, "subfolder": "", "type": "output"}]}, "result": (body, info.language)}



class AIVSPointsToJson:
    """"320,240; 400,300" (pixels) -> the JSON text the SAM2 nodes want."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {"points": ("STRING", {"default": "0,0"})}}

    RETURN_TYPES = ("STRING",)
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, points):
        import json

        pts = []
        for part in str(points).replace("\n", ";").split(";"):
            bits = [b for b in part.replace(" ", "").split(",") if b]
            if len(bits) == 2:
                pts.append({"x": int(float(bits[0])), "y": int(float(bits[1]))})
        if not pts:
            raise RuntimeError("Give at least one point as x,y (pixels from the top-left corner).")
        return (json.dumps(pts),)


class AIVSLamaErase:
    """Erase what the mask covers (LaMa, Apache-2.0, TorchScript weights in models/inpaint). Works on a crop around the mask for sharp results."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {
            "image": ("IMAGE",),
            "mask": ("MASK",),
            "model_name": ("STRING", {"default": "big-lama.pt"}),
            "margin": ("INT", {"default": 128, "min": 16, "max": 512}),
        }}

    RETURN_TYPES = ("IMAGE",)
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, image, mask, model_name, margin):
        import folder_paths
        import torch.nn.functional as F

        import comfy.model_management as mm

        dev = mm.get_torch_device()
        path = os.path.join(folder_paths.models_dir, "inpaint", os.path.basename(model_name))
        model = torch.jit.load(path, map_location=dev).eval()
        results = []
        try:
            for i in range(image.shape[0]):
                img = image[i][..., :3].contiguous()
                m = mask[min(i, mask.shape[0] - 1)]
                if tuple(m.shape) != tuple(img.shape[:2]):
                    m = F.interpolate(m[None, None], size=tuple(img.shape[:2]), mode="nearest")[0, 0]
                m = (m > 0.5).float()
                if float(m.sum()) == 0.0:
                    results.append(img)
                    continue
                ys, xs = torch.where(m > 0.5)
                h_all, w_all = m.shape
                y0, y1 = max(int(ys.min()) - margin, 0), min(int(ys.max()) + margin + 1, h_all)
                x0, x1 = max(int(xs.min()) - margin, 0), min(int(xs.max()) + margin + 1, w_all)
                crop = img[y0:y1, x0:x1].permute(2, 0, 1)[None].to(dev)
                k = m[y0:y1, x0:x1][None, None].to(dev)
                h, w = crop.shape[2], crop.shape[3]
                ph, pw = (-h) % 8, (-w) % 8
                if ph or pw:
                    crop_p = F.pad(crop, (0, pw, 0, ph), mode="replicate")
                    k_p = F.pad(k, (0, pw, 0, ph), mode="constant", value=0)
                else:
                    crop_p, k_p = crop, k
                with torch.no_grad():
                    out = model(crop_p, k_p)[:, :, :h, :w].clamp(0, 1)
                comp = crop * (1 - k) + out * k
                res = img.clone()
                res[y0:y1, x0:x1] = comp[0].permute(1, 2, 0).cpu()
                results.append(res)
        finally:
            del model
            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        return (torch.stack(results),)


class AIVSFaceRestore:
    """Restore faces in a photo (GFPGAN weights via spandrel, face finding via facexlib). Pictures without a face come back unchanged."""

    @classmethod
    def INPUT_TYPES(cls):
        return {"required": {
            "image": ("IMAGE",),
            "model_name": ("STRING", {"default": "GFPGANv1.4.pth"}),
            "blend": ("FLOAT", {"default": 1.0, "min": 0.0, "max": 1.0, "step": 0.05}),
        }}

    RETURN_TYPES = ("IMAGE",)
    FUNCTION = "run"
    CATEGORY = "AIVS"

    def run(self, image, model_name, blend):
        import cv2
        import folder_paths
        from facexlib.utils.face_restoration_helper import FaceRestoreHelper
        from spandrel import ModelLoader

        import comfy.model_management as mm

        dev = mm.get_torch_device()
        path = os.path.join(folder_paths.models_dir, "facerestore_models", os.path.basename(model_name))
        net = ModelLoader().load_from_file(path).model.to(dev).eval()
        root = os.path.join(folder_paths.models_dir, "facexlib")
        os.makedirs(root, exist_ok=True)
        results = []
        try:
            for i in range(image.shape[0]):
                rgb = (image[i][..., :3].cpu().numpy() * 255.0).clip(0, 255).astype(np.uint8)
                bgr = cv2.cvtColor(rgb, cv2.COLOR_RGB2BGR)
                helper = FaceRestoreHelper(1, face_size=512, crop_ratio=(1, 1), det_model="retinaface_resnet50", save_ext="png", use_parse=True, device=dev, model_rootpath=root)
                helper.read_image(bgr)
                if helper.get_face_landmarks_5(only_center_face=False, resize=640, eye_dist_threshold=5) == 0:
                    results.append(image[i][..., :3])
                    continue
                helper.align_warp_face()
                for face in helper.cropped_faces:
                    t = torch.from_numpy(face[:, :, ::-1].copy()).float().permute(2, 0, 1)[None] / 255.0
                    t = ((t - 0.5) / 0.5).to(dev)
                    with torch.no_grad():
                        o = net(t)
                        o = o[0] if isinstance(o, (tuple, list)) else o
                    o = ((o.clamp(-1, 1) + 1) / 2)[0].permute(1, 2, 0).cpu().numpy()
                    helper.add_restored_face((o[:, :, ::-1] * 255).round().astype(np.uint8))
                helper.get_inverse_affine(None)
                out = helper.paste_faces_to_input_image()
                out = cv2.cvtColor(out, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0
                orig = image[i][..., :3].cpu().numpy()
                results.append(torch.from_numpy(orig * (1 - blend) + out * blend).float())
        finally:
            del net
            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        return (torch.stack(results),)


NODE_CLASS_MAPPINGS = {
    "AIVSSaveVideoBase64": AIVSSaveVideoBase64,
    "AIVSLoadImageBase64": AIVSLoadImageBase64,
    "AIVSLoadAudioBase64": AIVSLoadAudioBase64,
    "AIVSLoadVideoBase64": AIVSLoadVideoBase64,
    "AIVSTranscribeSubtitles": AIVSTranscribeSubtitles,
    "AIVSPointsToJson": AIVSPointsToJson,
    "AIVSLamaErase": AIVSLamaErase,
    "AIVSFaceRestore": AIVSFaceRestore,
}
NODE_DISPLAY_NAME_MAPPINGS = {
    "AIVSSaveVideoBase64": "AIVS Save Video (inline)",
    "AIVSLoadImageBase64": "AIVS Load Image (inline)",
    "AIVSLoadAudioBase64": "AIVS Load Audio (inline)",
    "AIVSLoadVideoBase64": "AIVS Load Video (inline)",
    "AIVSTranscribeSubtitles": "AIVS Speech to Subtitles",
    "AIVSPointsToJson": "AIVS Points to JSON",
    "AIVSLamaErase": "AIVS Erase (LaMa)",
    "AIVSFaceRestore": "AIVS Restore Faces",
}
