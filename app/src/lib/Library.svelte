<script lang="ts">
  import Chain from "./Chain.svelte";
  let chain = $state(false);
  let { embedded = false }: { embedded?: boolean } = $props();
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, type Job } from "./api";
  import { S } from "./store.svelte";

  let clips = $derived(S.jobs.filter((j) => j.status === "done" && j.project_id === S.projectId && j.file_path).sort((a, b) => b.finished_at - a.finished_at));
  let open = $state<Job | null>(null);
  let confirmDel = $state(false);

  const src = (j: Job) => convertFileSrc(j.file_path);
  const isAud = (j: Job) => /\.(mp3|flac|wav|ogg|opus)$/i.test(j.file_path);
  const isTxt = (j: Job) => /\.(srt|vtt)$/i.test(j.file_path);
  const is3d = (j: Job) => /\.(glb|obj|ply)$/i.test(j.file_path);
  let text = $state("");
  $effect(() => { text = ""; if (open && isTxt(open)) api.textOutput(open.file_path).then((t) => (text = t)).catch((x) => (text = String(x))); });
  const isImg = (j: Job) => /\.(png|jpe?g|webp|gif)$/i.test(j.file_path);
  const wfName = (id: string) => { const w = S.workflows.find((x) => x.meta.id === id); return w ? (S.lang === "el" ? w.meta.name_el : w.meta.name_en) : id; };
  const when = (t: number) => new Date(t * 1000).toLocaleString(S.lang === "el" ? "el-GR" : "en-GB");

  async function del() {
    if (!open) return;
    const id = open.id;
    await S.run(() => api.deleteJob(id, true));
    S.jobs = S.jobs.filter((j) => j.id !== id);
    open = null; confirmDel = false;
  }
  async function copy() {
    await navigator.clipboard.writeText(open!.prompt);
    S.notify(S.t("copied"));
  }
  function reuse() {
    S.draft = { prompt: open!.prompt, workflow: open!.workflow_id };
    open = null; S.page = "generate";
  }
</script>

<div class={embedded ? "" : "page"}>
  <div class="row">
    <h1 class="grow">{S.t("nav_library")} <span class="dim small">· {S.project?.name}</span></h1>
    <button onclick={() => S.run(() => api.reveal(S.project!.dir))}>{S.t("open_folder")}</button>
  </div>

  {#if clips.length === 0}
    <div class="empty"><p class="dim">{S.t("library_empty")}</p><button class="primary" onclick={() => (S.page = "generate")}>✦ {S.t("nav_generate")}</button></div>
  {:else}
    <div class="grid">
      {#each clips as j (j.id)}
        <button class="clip" onclick={() => { open = j; confirmDel = false; }}>
          {#if isImg(j)}
            <img src={src(j)} alt={j.prompt} />
          {:else if isAud(j)}
            <div class="aud">♪</div>
          {:else if isTxt(j)}
            <div class="aud">CC</div>
          {:else if is3d(j)}
            <div class="aud">3D</div>
          {:else}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video src={src(j) + "#t=0.1"} preload="metadata" muted></video>
          {/if}
          <div class="cap">{j.prompt}</div>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if open}
  <div class="backdrop" role="presentation" onclick={() => (open = null)}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      {#if isImg(open)}
        <img class="full" src={src(open)} alt={open.prompt} />
      {:else if isTxt(open)}
        <pre class="subs">{text}</pre>
        <div class="row"><button onclick={() => navigator.clipboard.writeText(text).then(() => S.notify(S.t("copied")))}>{S.t("copy_prompt").replace(/.*/, S.lang === "el" ? "Αντιγραφή υποτίτλων" : "Copy subtitles")}</button></div>
      {:else if is3d(open)}
        <p class="dim">{S.lang === "el" ? "Αρχείο 3D. Άνοιξέ το με το πρόγραμμα 3D της επιλογής σου (π.χ. Windows 3D Viewer, Blender)." : "3D file. Open it with your 3D viewer (for example Windows 3D Viewer or Blender)."}</p>
      {:else if isAud(open)}
        <!-- svelte-ignore a11y_media_has_caption -->
        <audio src={src(open)} controls autoplay style="width:100%"></audio>
      {:else}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video src={src(open)} controls autoplay loop></video>
      {/if}
      <p style="margin-top:12px">{open.prompt}</p>
      {#if chain}{#key open.id}<Chain job={open} onclose={() => { chain = false; open = null; }} />{/key}{/if}
      <p class="small dim">{wfName(open.workflow_id)} · {when(open.finished_at)}{#if open.est_cost > 0} · {S.t("cost_actual")} ${open.est_cost.toFixed(3)}{/if}</p>
      <div class="row wrap">
        <button onclick={() => S.run(() => api.reveal(open!.file_path))}>{S.t("open_folder")}</button>
        <button onclick={copy}>{S.t("copy_prompt")}</button>
        <button onclick={reuse}>{S.t("reuse_prompt")}</button>
        <button onclick={() => (chain = !chain)}>{S.lang === "el" ? "Φτιάξε κάτι ακόμα…" : "Make more from this…"}</button>
        {#if confirmDel}
          <button class="danger" onclick={del}>{S.t("confirm_delete")}</button>
        {:else}
          <button class="danger" onclick={() => (confirmDel = true)}>{S.t("delete_clip")}</button>
        {/if}
        <button class="ghost" style="margin-left:auto" onclick={() => (open = null)}>{S.t("close")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 14px; margin-top: 14px; }
  .empty { padding: 48px 0; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
  .subs { white-space: pre-wrap; max-height: 50vh; overflow: auto; background: var(--panel2); padding: 10px; border-radius: 8px; font-size: 13px; }
  .aud { display: grid; place-items: center; height: 120px; font-size: 40px; color: var(--dim); background: var(--panel2); }
  .clip { padding: 0; overflow: hidden; text-align: left; background: var(--panel); border-radius: 10px; display: block; }
  .clip img { width: 100%; aspect-ratio: 16/10; object-fit: cover; display: block; background: #000; pointer-events: none; }
  .modal img.full { width: 100%; max-height: 62vh; object-fit: contain; background: #000; border-radius: 8px; }
  .clip video { width: 100%; aspect-ratio: 16/10; object-fit: cover; display: block; background: #000; pointer-events: none; }
  .cap { padding: 8px 10px; font-size: 12px; color: var(--dim); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .backdrop { position: fixed; inset: 0; background: rgba(0,0,0,.7); display: grid; place-items: center; z-index: 10; }
  .modal { background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 16px; width: min(900px, 92vw); max-height: 92vh; overflow-y: auto; }
  .modal video { width: 100%; max-height: 62vh; background: #000; border-radius: 8px; }
</style>
