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


NODE_CLASS_MAPPINGS = {
    "AIVSSaveVideoBase64": AIVSSaveVideoBase64,
    "AIVSLoadImageBase64": AIVSLoadImageBase64,
    "AIVSLoadAudioBase64": AIVSLoadAudioBase64,
    "AIVSLoadVideoBase64": AIVSLoadVideoBase64,
}
NODE_DISPLAY_NAME_MAPPINGS = {
    "AIVSSaveVideoBase64": "AIVS Save Video (inline)",
    "AIVSLoadImageBase64": "AIVS Load Image (inline)",
    "AIVSLoadAudioBase64": "AIVS Load Audio (inline)",
    "AIVSLoadVideoBase64": "AIVS Load Video (inline)",
}
