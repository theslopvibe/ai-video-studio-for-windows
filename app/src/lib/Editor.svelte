<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import { S } from "./store.svelte";

  const el = $derived(S.lang === "el");
  let tab = $state<"video" | "image">("video");
  let url = $state<string | null>(null);
  let paintUrl = $state<string | null>(null);
  let tries = $state(0);
  let paintFrame = $state<HTMLIFrameElement>();

  // the local editor servers start with the app; wait a moment if they are not up yet
  onMount(() => {
    (async () => {
      while ((!url || !paintUrl) && tries < 20) {
        url ??= await api.editorUrl().catch(() => null);
        paintUrl ??= await api.paintUrl().catch(() => null);
        if (!url || !paintUrl) { tries++; await new Promise((r) => setTimeout(r, 300)); }
      }
    })();
    const on = (ev: MessageEvent) => { if (paintFrame && ev.source === paintFrame.contentWindow) void handleAi(ev.data); };
    window.addEventListener("message", on);
    return () => window.removeEventListener("message", on);
  });

  // Image editor → app: the AI menu inside the editor asks for a picture, we run a job and answer with the file.
  async function handleAi(msg: { avs?: string; id: string; op: string; prompt?: string; width?: number; height?: number }) {
    if (msg?.avs !== "ai") return;
    const reply = (r: { image?: string; error?: string }) => paintFrame?.contentWindow?.postMessage({ avs: "ai-result", id: msg.id, ...r }, "*");
    try {
      if (msg.op !== "generate") throw new Error(el ? "Αυτό το εργαλείο δεν υπάρχει ακόμα." : "This tool is not available yet.");
      const wf = S.workflows.filter((w) => w.meta.kind === "t2i").sort((a, b) => (b.meta.priority ?? 0) - (a.meta.priority ?? 0))[0];
      if (!wf) throw new Error(el ? "Δεν υπάρχει μοντέλο εικόνας. Πρόσθεσε ένα στα Models." : "No image model found. Add one in Models.");
      const keys = new Set(wf.meta.inputs.map((i) => i.key));
      const snap = (n: number) => Math.min(1536, Math.max(256, Math.round(n / 64) * 64));
      const params: Record<string, unknown> = {};
      if (keys.has("width")) params.width = snap(msg.width ?? 1024);
      if (keys.has("height")) params.height = snap(msg.height ?? 1024);
      const job = await api.enqueue(S.projectId, wf.meta.id, String(msg.prompt ?? ""), params);
      for (;;) {
        await new Promise((r) => setTimeout(r, 1500));
        const j = (await api.jobs()).find((x) => x.id === job.id);
        if (!j) throw new Error(el ? "Η εργασία χάθηκε." : "The job disappeared.");
        if (j.status === "failed" || j.status === "cancelled") throw new Error(j.error || j.message);
        if (j.status === "done") return reply({ image: await api.imageDataUrl(j.file_path) });
      }
    } catch (x) {
      reply({ error: String((x as Error)?.message ?? x) });
    }
  }
</script>

<div class="ed">
  <div class="bar">
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === "video"} class="seg" class:on={tab === "video"} onclick={() => (tab = "video")}>{el ? "Video" : "Video"}</button>
      <button role="tab" aria-selected={tab === "image"} class="seg" class:on={tab === "image"} onclick={() => (tab = "image")}>{el ? "Εικόνα" : "Image"}</button>
    </div>
    <span class="small dim">
      {#if tab === "video"}
        {el ? "Επεξεργασία video: το" : "Video editor:"}
        <a href="https://github.com/opencut-app/opencut-classic" target="_blank" rel="noreferrer">OpenCut</a>
      {:else}
        {el ? "Επεξεργασία εικόνας: το" : "Image editor:"}
        <a href="https://github.com/viliusle/miniPaint" target="_blank" rel="noreferrer">miniPaint</a>
      {/if}
      {el ? "(ανοιχτός κώδικας, άδεια MIT), τρέχει τοπικά στον υπολογιστή σου." : "(open source, MIT license), running locally on your computer."}
    </span>
    <span class="grow"></span>
    <span class="small dim">{el ? "Τα clips σου:" : "Your clips:"}</span>
    <button class="ghost small" onclick={() => S.run(() => api.reveal(S.boot?.output_dir ?? ""))}>{el ? "Άνοιγμα φακέλου" : "Open output folder"}</button>
  </div>
  {#if url}
    <iframe title="OpenCut" class:hide={tab !== "video"} src={url + "/projects/"} allow="clipboard-read; clipboard-write; fullscreen"></iframe>
  {/if}
  {#if paintUrl}
    <iframe title="miniPaint" class:hide={tab !== "image"} bind:this={paintFrame} src={paintUrl + "/"} allow="clipboard-read; clipboard-write; fullscreen"></iframe>
  {/if}
  {#if !url && !paintUrl && tries >= 20}
    <p class="msg err" style="margin:16px">{el ? "Ο editor δεν βρέθηκε σε αυτή την έκδοση της εφαρμογής." : "The editor is not part of this build of the app."}</p>
  {:else}
    <p class="dim" style="margin:16px">…</p>
  {/if}
</div>

<style>
  .ed { display: flex; flex-direction: column; height: 100%; }
  .bar { display: flex; gap: 8px; align-items: center; padding: 6px 12px; border-bottom: 1px solid var(--line); background: var(--panel); }
  .grow { flex: 1; }
  .tabs { display: flex; gap: 4px; }
  .seg { background: transparent; border-radius: 99px; padding: 4px 12px; color: var(--dim); }
  .seg.on { background: var(--panel2, var(--line)); color: var(--text, inherit); }
  .hide { display: none; }
  iframe { flex: 1; border: 0; width: 100%; min-height: 0; background: #0e0e0e; }
</style>
