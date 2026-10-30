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


NODE_CLASS_MAPPINGS = {"AIVSSaveVideoBase64": AIVSSaveVideoBase64, "AIVSLoadImageBase64": AIVSLoadImageBase64}
NODE_DISPLAY_NAME_MAPPINGS = {
    "AIVSSaveVideoBase64": "AIVS Save Video (inline)",
    "AIVSLoadImageBase64": "AIVS Load Image (inline)",
}
