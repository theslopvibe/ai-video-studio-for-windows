// Writes the InfiniteTalk avatar workflows (src-tauri/workflows/avatar-infinitetalk-{480,720}.json) and nothing else.
// Chunk 1 is written out here; the app (workflows.rs extend_chunks) clones chunk 2..n at run time from the `seconds` input.
import fs from "node:fs";
const out = (f, o) => fs.writeFileSync(new URL(`../src-tauri/workflows/${f}`, import.meta.url), JSON.stringify(o, null, 1) + "\n");
const sizes = {
  480: { w: 832, h: 480, unet: "Wan2_1-I2V-14B-480p_fp8_e4m3fn_scaled_KJ.safetensors", vram: 24, est: 600 },
  720: { w: 1280, h: 720, unet: "Wan2_1-I2V-14B-720p_fp8_e4m3fn_scaled_KJ.safetensors", vram: 32, est: 1500 },
};
for (const [p, s] of Object.entries(sizes)) {
  const meta = {
    name_en: `Photo + Voice → Talking avatar (InfiniteTalk ${p}p, experimental)`,
    name_el: `Φωτογραφία + Φωνή → Avatar που μιλάει (InfiniteTalk ${p}p, πειραματικό)`,
    desc_en: `Makes a person in a photo talk, with the lips, face and head moving to a voice recording of any length (HeyGen style). Give the length of the speech in seconds. ${p}p output; slow (several minutes per 10 seconds of video) and needs a big GPU. Only use faces and voices you have permission to use.`,
    desc_el: `Κάνει ένα πρόσωπο σε φωτογραφία να μιλά, με στόμα, πρόσωπο και κεφάλι που κινούνται στη φωνή μιας ηχογράφησης οποιουδήποτε μήκους (στυλ HeyGen). Δώσε τη διάρκεια της ομιλίας σε δευτερόλεπτα. Έξοδος ${p}p· αργό και θέλει μεγάλη GPU. Μόνο πρόσωπα και φωνές με άδεια.`,
    good_en: "Talking-head YouTube videos from a photo and a script (voice from the TTS / clone modes).",
    good_el: "YouTube βίντεο με avatar που μιλά, από φωτογραφία και κείμενο (η φωνή από τα TTS / clone).",
    notice_en: "Only use faces and voices you have permission to use. Do not make people appear to say things they did not say.",
    notice_el: "Χρησιμοποίησε μόνο πρόσωπα και φωνές με άδεια. Μην κάνεις ανθρώπους να φαίνονται ότι λένε πράγματα που δεν είπαν.",
    kind: "lipsync", license: "Apache-2.0", pack: `infinitetalk-${p}`, vram_gb: s.vram, est_sec: s.est, priority: p == 720 ? 230 : 235,
    repeat_seconds: "seconds",
    inputs: [
      { key: "prompt", type: "text", required: true, label_en: "How the person looks and acts (short)", label_el: "Πώς φαίνεται και συμπεριφέρεται (σύντομα)", default: "A person talks to the camera, natural expression and head movement." },
      { key: "image", type: "image", required: true, label_en: "Photo of the person (face to camera)", label_el: "Φωτογραφία του προσώπου (de face)" },
      { key: "audio", type: "audio", required: true, label_en: "Voice recording", label_el: "Ηχογράφηση φωνής" },
      { key: "seconds", type: "int", default: 10, min: 3, max: 180, step: 1, label_en: "Length of the speech in seconds", label_el: "Διάρκεια ομιλίας σε δευτερόλεπτα" },
      { key: "seed", type: "seed", default: "__RANDOM_INT__", advanced: true, label_en: "Seed", label_el: "Seed" },
    ],
    id: `avatar-infinitetalk-${p}`,
  };
  const wf = {
    unet: { class_type: "UNETLoader", inputs: { unet_name: s.unet, weight_dtype: "default" } },
    clip: { class_type: "CLIPLoader", inputs: { clip_name: "umt5_xxl_fp8_e4m3fn_scaled.safetensors", type: "wan", device: "default" } },
    vae: { class_type: "VAELoader", inputs: { vae_name: "Wan2_1_VAE_bf16.safetensors" } },
    lora: { class_type: "LoraLoaderModelOnly", inputs: { model: ["unet", 0], lora_name: "lightx2v_I2V_14B_480p_cfg_step_distill_rank64_bf16.safetensors", strength_model: 1 } },
    patch: { class_type: "ModelPatchLoader", inputs: { name: "wan2.1_infiniteTalk_single_fp16.safetensors" } },
    aenc: { class_type: "AudioEncoderLoader", inputs: { audio_encoder_name: "wav2vec2-chinese-base_fp16.safetensors" } },
    img: { class_type: "AIVSLoadImageBase64", inputs: { image_b64: "{{image}}" } },
    aud: { class_type: "AIVSLoadAudioBase64", inputs: { audio_b64: "{{audio}}" } },
    aout: { class_type: "AudioEncoderEncode", inputs: { audio_encoder: ["aenc", 0], audio: ["aud", 0] } },
    pos: { class_type: "CLIPTextEncode", inputs: { clip: ["clip", 0], text: "{{prompt}}" } },
    neg: { class_type: "ConditioningZeroOut", inputs: { conditioning: ["pos", 0] } },
    sampler: { class_type: "KSamplerSelect", inputs: { sampler_name: "euler" } },
    talk: { class_type: "WanInfiniteTalkToVideo", inputs: { mode: "single_speaker", model: ["lora", 0], model_patch: ["patch", 0], positive: ["pos", 0], negative: ["neg", 0], vae: ["vae", 0], width: s.w, height: s.h, length: 81, start_image: ["img", 0], audio_encoder_output_1: ["aout", 0], motion_frame_count: 9, audio_scale: 1 } },
    guide: { class_type: "CFGGuider", inputs: { model: ["talk", 0], positive: ["talk", 1], negative: ["talk", 2], cfg: 1 } },
    sched: { class_type: "BasicScheduler", inputs: { model: ["talk", 0], scheduler: "normal", steps: 6, denoise: 1 } },
    noise: { class_type: "RandomNoise", inputs: { noise_seed: "{{seed}}" } },
    samp: { class_type: "SamplerCustomAdvanced", inputs: { noise: ["noise", 0], guider: ["guide", 0], sampler: ["sampler", 0], sigmas: ["sched", 0], latent_image: ["talk", 3] } },
    dec: { class_type: "VAEDecode", inputs: { samples: ["samp", 0], vae: ["vae", 0] } },
    video: { class_type: "CreateVideo", inputs: { images: ["dec", 0], audio: ["aud", 0], fps: 25 } },
    save: { class_type: "SaveVideo", inputs: { video: ["video", 0], filename_prefix: "video/avatar", format: "mp4", codec: "auto" } },
  };
  out(`avatar-infinitetalk-${p}.json`, { meta, workflow: wf });
  if (p == 480) {
    // RunPod twin: same graph; the app turns the inline photo/voice nodes into LoadImage/LoadAudio files for the worker
    const twin = { ...meta, id: "runpod-avatar-infinitetalk-480", name_en: "RunPod · " + meta.name_en.replace(", experimental", ""), name_el: "RunPod · " + meta.name_el.replace(", πειραματικό", ""),
      models: [], pack: "", vram_gb: 48, est_sec: 600, est_usd: 0.15, priority: 165, provider: "runpod", model: "", request: {}, license: "paid" };
    out("runpod-avatar-infinitetalk-480.json", { meta: twin, workflow: wf });
  }
}
