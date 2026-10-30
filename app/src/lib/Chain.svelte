<script lang="ts">
  import { open as pickDialog } from "@tauri-apps/plugin-dialog";
  import { api, type Job, type InputDef } from "./api";
  import { S } from "./store.svelte";

  let { job, kinds = [], onclose }: { job?: Job; kinds?: string[]; onclose: () => void } = $props();
  const el = $derived(S.lang === "el");

  const kindOf = (path: string): "image" | "audio" | "video" =>
    /\.(png|jpe?g|webp|gif)$/i.test(path) ? "image" : /\.(mp3|flac|wav|ogg|opus|m4a)$/i.test(path) ? "audio" : "video";
  const mine = $derived(job ? kindOf(job.file_path) : "");
  const MEDIA = ["image", "audio", "video"];
  const EXT: Record<string, string[]> = { image: ["png", "jpg", "jpeg", "webp"], audio: ["mp3", "wav", "flac", "ogg", "opus", "m4a"], video: ["mp4", "webm", "mov", "mkv"] };

  // every mode that takes this kind of result as input is an action: new models show up here without code changes
  const options = $derived(S.workflows.filter((w) => (job ? w.meta.inputs.some((i) => i.type === mine) : kinds.includes(w.meta.kind))));
  let wfId = $state("");
  const wf = $derived(options.find((w) => w.meta.id === wfId) ?? options[0]);
  const name = (w: { meta: { name_el: string; name_en: string } }) => (el ? w.meta.name_el : w.meta.name_en);
  const label = (i: InputDef) => (el ? i.label_el : i.label_en) || i.key;
  const slots = $derived((wf?.meta.inputs ?? []).filter((i) => MEDIA.includes(i.type)));
  const fields = $derived((wf?.meta.inputs ?? []).filter((i) => !MEDIA.includes(i.type) && i.key !== "prompt" && !i.advanced));
  const hasPrompt = $derived((wf?.meta.inputs ?? []).some((i) => i.key === "prompt"));
  let prompt = $state(job?.prompt ?? "");
  let vals = $state<Record<string, unknown>>({});
  let src = $state<Record<string, string>>({}); // slot key -> "job:ID" | "file:PATH" | ""
  let busy = $state(false);
  let msg = $state("");

  // the first slot of the result's own type gets this result; the rest start empty
  $effect(() => {
    if (!wf) return;
    const first = slots.find((s) => s.type === mine);
    const next: Record<string, string> = {};
    for (const s of slots) next[s.key] = job && s === first ? `job:${job.id}` : "";
    src = next;
    vals = {};
  });

  const library = (type: string) => S.jobs.filter((j) => j.status === "done" && j.file_path && kindOf(j.file_path) === type).sort((a, b) => b.finished_at - a.finished_at);
  async function browse(s: InputDef) {
    const f = await pickDialog({ multiple: false, filters: [{ name: s.type, extensions: EXT[s.type] }] });
    if (typeof f === "string") S.run(async () => { src[s.key] = await api.inputImport(f); });
  }
  const val = (i: InputDef) => vals[i.key] ?? (i.default === "__RANDOM_INT__" ? "" : i.default ?? "");

  async function go() {
    if (!wf) return;
    const missing = slots.find((s) => s.required && !src[s.key]);
    if (missing) { msg = (el ? "Λείπει: " : "Missing: ") + label(missing); return; }
    busy = true; msg = "";
    try {
      const params: Record<string, unknown> = {};
      for (const s of slots) if (src[s.key]) params[s.key] = src[s.key];
      for (const f of fields) { const v = val(f); params[f.key] = f.type === "int" || f.type === "float" ? Number(v) : v; }
      await api.enqueue(S.projectId, wf.meta.id, (hasPrompt ? prompt : "") || name(wf), params);
      onclose();
      if (job) S.page = "library";
      S.jobs = await api.jobs();
    } catch (x) { msg = String(x); }
    busy = false;
  }
</script>

<div class="chain">
  {#if job}<h3>{el ? "Φτιάξε κάτι ακόμα από αυτό" : "Make something more from this"}</h3>{/if}
  {#if !options.length}
    <p class="dim">{el ? "Δεν υπάρχει ακόμα εργαλείο για αυτό το αρχείο." : "No tool takes this kind of file yet."}</p>
  {:else}
    <label for="chain-wf">{el ? "Τι να γίνει" : "What to do"}</label>
    <select id="chain-wf" bind:value={wfId}>
      {#each options as w (w.meta.id)}<option value={w.meta.id}>{name(w)}</option>{/each}
    </select>
    {#if wf}
      <p class="small dim">{el ? wf.meta.desc_el : wf.meta.desc_en}</p>
      {#if wf.meta.languages?.length}<p class="small dim">{el ? "Γλώσσες: " : "Languages: "}{wf.meta.languages.join(", ")}</p>{/if}
      {#if wf.meta.notice_en}<p class="msg err small">{el ? wf.meta.notice_el : wf.meta.notice_en}</p>{/if}
      {#each slots as s (s.key)}
        <label for={"s-" + s.key}>{label(s)}{s.required ? " *" : ""}</label>
        <div class="row">
          <select id={"s-" + s.key} bind:value={src[s.key]}>
            <option value="">{el ? "— καμία —" : "— none —"}</option>
            {#if src[s.key]?.startsWith("file:")}<option value={src[s.key]}>{el ? "Αρχείο που διάλεξα" : "File I picked"}</option>{/if}
            {#each library(s.type) as j (j.id)}<option value={"job:" + j.id}>{j.id === job?.id ? (el ? "Αυτό το αποτέλεσμα" : "This result") : j.prompt.slice(0, 50)}</option>{/each}
          </select>
          <button onclick={() => browse(s)}>{el ? "Αρχείο…" : "Browse…"}</button>
        </div>
      {/each}
      {#if hasPrompt}
        <label for="chain-prompt">{el ? "Περιγραφή" : "Text"}</label>
        <textarea id="chain-prompt" rows="3" bind:value={prompt}></textarea>
      {/if}
      {#each fields as f (f.key)}
        <label for={"f-" + f.key}>{label(f)}</label>
        {#if f.options?.length}
          <select id={"f-" + f.key} value={String(val(f))} onchange={(e) => (vals[f.key] = e.currentTarget.value)}>
            {#each f.options as o}<option value={o}>{o}</option>{/each}
          </select>
        {:else if f.type === "text"}
          <input id={"f-" + f.key} type="text" value={String(val(f))} oninput={(e) => (vals[f.key] = e.currentTarget.value)} />
        {:else}
          <input id={"f-" + f.key} type="number" value={String(val(f))} min={f.min} max={f.max} step={f.step} oninput={(e) => (vals[f.key] = e.currentTarget.value)} />
        {/if}
      {/each}
    {/if}
    {#if msg}<p class="msg err">{msg}</p>{/if}
    <div class="row">
      <button class="primary" disabled={busy || !wf} onclick={go}>{el ? "Στην ουρά" : "Add to queue"}</button>
      {#if job}<button class="ghost" onclick={onclose}>{el ? "Άκυρο" : "Cancel"}</button>{/if}
    </div>
  {/if}
</div>

<style>
  .chain { display: flex; flex-direction: column; gap: 6px; padding-top: 8px; }
  .chain label { font-size: 12px; color: var(--dim); margin-top: 6px; }
  .row { display: flex; gap: 8px; align-items: center; }
  .row select { flex: 1; }
</style>
