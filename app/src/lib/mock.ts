// Dev-only fake backend so the UI can be built and screenshotted in a plain browser: `npm run dev`, open /?mock
// Never bundled into the app: it is only imported behind `import.meta.env.DEV`.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import models from "../../src-tauri/models.json";

const wf = (id: string, kind: string, pack: string, priority: number) => ({
  meta: { id, name_el: id, name_en: id, desc_el: "", desc_en: "Mock workflow", kind, models: [], vram_gb: 12, est_sec: 60, pack, priority, inputs: [] },
  source: "bundled", file: id + ".json",
});

const gpuNvidia = { os: "windows x86_64", gpus: [{ name: "NVIDIA GeForce RTX 4060", vram_mb: 8188, driver: "576.02" }], ram_gb: 34.1, disk_free_gb: 408.6, disk_path: "C:\\", support: "nvidia" };
const state = { installed: false, settings: {} as Record<string, string> };

const chat = {
  folders: [{ id: "f0", name: "Fox story" }] as { id: string; name: string }[],
  chats: [{ id: "c0", folder_id: "f0", title: "Fox story ideas", provider: "claude_sub", model: "sonnet", system: "", pinned: true, created_at: 0, updated_at: 0, mode: "chat", snippet: "" }] as { id: string; folder_id: string; title: string; provider: string; model: string; system: string; pinned: boolean; created_at: number; updated_at: number; mode: string; snippet: string }[],
  msgs: { c0: [{ id: "m1", role: "user", content: "Give me three video ideas about a fox in winter.", created_at: 0 }, { id: "m2", role: "assistant", content: "1. A fox listens for a mouse under the snow, then pounces.\n2. A fox crosses a frozen river at dawn.\n3. A fox curls up as snow falls.", created_at: 0 }] } as Record<string, { id: string; role: string; content: string; created_at: number }[]>,
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function fakeDownload(steps: [string, string][]) {
  for (const [step, detail] of steps) {
    for (let p = 0; p <= 100; p += 10) {
      await emit("setup-progress", { step, pct: p, detail: `${detail} · ${(p / 100 * 2.04).toFixed(1)} / 2.0 GB` });
      await sleep(120);
    }
  }
}

export function installMock(hw: unknown = gpuNvidia) {
  mockIPC(async (cmd, args) => {
    switch (cmd) {
      case "bootstrap":
        return {
          projects: [{ id: "p1", name: "Default", created_at: 0, folder: "", suffix: "", sidecar: true, dir: "C:\\Videos" }],
          secrets: { vast_api_key: false }, settings: state.settings,
          workflows: [wf("ltxv-t2v", "t2v", "ltxv-2b", 40), wf("wan22-t2v-lightning", "t2v", "wan22-t2v", 10)],
          workflow_errors: [], user_workflows_dir: "", output_dir: "C:\\Videos", anthropic_models: [], cli_models: [], claude_cli: null, models,
        };
      case "jobs_list": return [];
      case "gpu_offers": return [];
      case "enhance_prompt": return { text: "mock", tokens: 1, provider: "mock" };
      case "hardware_scan": await sleep(500); return hw;
      case "local_status": return { installed: state.installed, running: false, dir: "C:\\app\\comfyui", port: 8188, models: [] };
      case "local_install":
        await fakeDownload([["tools", "7zr.exe"], ["comfy", "ComfyUI_windows_portable_nvidia.7z"], ["extract", "unpacking"]]);
        state.installed = true;
        return;
      case "local_start": await sleep(800); return;
      case "local_install_models": await fakeDownload([["models", "ltx-video-2b-v0.9.5.safetensors"]]); return;
      case "setting_set": state.settings[(args as { key: string }).key] = (args as { value: string }).value; return;
      case "secret_set": return { vast_api_key: true };
      case "test_vast": return { ok: true, message: "ok", endpoint: null, all_endpoints: [] };
      case "job_enqueue": return { id: "j1" };
      case "log_ui": return;
      case "chat_providers": return [
        { id: "claude_sub", label: "Claude (subscription via Claude Code)", model: "sonnet", needs_key: false, has_key: true },
        { id: "chatgpt_sub", label: "ChatGPT (subscription via Codex CLI)", model: "", needs_key: true, has_key: false },
        { id: "gemini_sub", label: "Gemini (subscription via Gemini CLI)", model: "", needs_key: true, has_key: false },
        { id: "openai", label: "OpenAI GPT", model: "gpt-4o-mini", needs_key: true, has_key: false },
        { id: "glm", label: "Z.ai GLM", model: "glm-4.6", needs_key: true, has_key: false },
      ];
      case "chat_models": return ["sonnet", "haiku", "opus"];
      case "chat_folders": return chat.folders;
      case "chat_folder_create": chat.folders.push({ id: "f" + chat.folders.length, name: (args as { name: string }).name }); return chat.folders;
      case "chat_list": {
        const q = ((args as { query: string }).query ?? "").toLowerCase();
        return q ? chat.chats.filter((c) => c.title.toLowerCase().includes(q) || (chat.msgs[c.id] ?? []).some((m) => m.content.toLowerCase().includes(q))).map((c) => ({ ...c, snippet: "…" + q + "…" })) : chat.chats;
      }
      case "chat_new": {
        const c = { id: "c" + chat.chats.length, folder_id: "", title: "New chat", provider: (args as { provider: string }).provider, model: "sonnet", system: "", pinned: false, created_at: 0, updated_at: 0, mode: (args as { mode?: string }).mode ?? "chat", snippet: "" };
        chat.chats.unshift(c); chat.msgs[c.id] = []; return c;
      }
      case "chat_messages": return chat.msgs[(args as { id: string }).id] ?? [];
      case "chat_update": return;
      case "chat_send": {
        const { id, text } = args as { id: string; text: string };
        await sleep(600);
        chat.msgs[id] = [...(chat.msgs[id] ?? []), { id: "u" + Math.random(), role: "user", content: text, created_at: 0 }, { id: "a" + Math.random(), role: "assistant", content: chat.chats.find((x) => x.id === id)?.mode === "studio" ? "LTX-Video is the fastest and cheapest here, good for a quick test.\n```video\n{\"workflow\":\"ltxv-t2v\",\"prompt\":\"A small fox pads through fresh snow in a pine forest at sunrise, slow camera follow, cinematic.\",\"params\":{}}\n```" : "A small fox pads through fresh snow in a pine forest at sunrise, soft golden light, slow camera follow, cinematic.", created_at: 0 }];
        const c = chat.chats.find((x) => x.id === id); if (c && c.title === "New chat") c.title = text.slice(0, 40);
        return chat.msgs[id];
      }
      default:
        if (cmd.startsWith("plugin:")) return;
        throw new Error("mock: unhandled command " + cmd);
    }
  }, { shouldMockEvents: true });
}
