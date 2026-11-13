<script lang="ts">
  import { api, type InputDef } from "./api";
  import { ct } from "./chatStrings";
  import { S } from "./store.svelte";
  import JobCard from "./JobCard.svelte";
  import Chain from "./Chain.svelte";
  let extra = $state<"" | "voice" | "tools">("");

  const isI2v = (w?: { meta: { inputs: InputDef[] } }) => !!w?.meta.inputs.some((i) => i.type === "image");
  let wfId = $state(S.settings.last_workflow || S.workflows[0]?.meta.id || "");
  let kind = $state<"t2v" | "i2v" | "t2i" | "t2a">(S.workflows.find((w) => w.meta.id === wfId)?.meta.kind === "t2a" ? "t2a" : S.workflows.find((w) => w.meta.id === wfId)?.meta.kind === "t2i" ? "t2i" : isI2v(S.workflows.find((w) => w.meta.id === wfId)) ? "i2v" : "t2v");
  type Img = { b64: string; url: string }; // b64 = JPEG without data: prefix
  type Scene = { prompt: string; imgs: Record<string, Img> };
  let imgs = $state<Record<string, Img>>({}); // by slot key: image (start), image_end (end)
  let scenes = $state<Scene[]>([]); // finished scenes waiting to be generated together
  let dragging = $state("");
  let runOn = $state(""); // "" = the default from Settings
  let prompt = $state("");
  let before = $state<string | null>(null);
  let batch = $state(false);
  let busy = $state(false);
  let showAdv = $state(false);
  let values = $state<Record<string, Record<string, unknown>>>({});

  // Where a job runs: "runpod" (cloud) or "local" (this PC). On RunPod an open-source mode is replaced by its RunPod twin.
  const where = $derived(runOn || S.provider);
  const hasTwin = (w: { meta: { id: string } }) => S.workflows.some((x) => x.meta.id === "runpod-" + w.meta.id && x.meta.provider === "runpod");
  const inScope = (w: { meta: { id: string; provider?: string } }) => (w.meta.provider === "runpod" ? where === "runpod" : w.meta.provider ? true : !(where === "runpod" && hasTwin(w)));
  const grpOf = (w: { meta: { provider?: string } }) => (w.meta.provider === "runpod" ? "runpod" : w.meta.provider ? "paid" : "open");
  const ORDER = $derived(where === "runpod" ? ["runpod", "paid", "open"] : ["open", "paid", "runpod"]);
  const rank = (w: { meta: { provider?: string } }) => ORDER.indexOf(grpOf(w));
  let list = $derived(S.workflows.filter((w) => inScope(w) && (kind === "t2a" ? w.meta.kind === "t2a" : kind === "t2i" ? w.meta.kind === "t2i" : w.meta.kind !== "t2i" && w.meta.kind !== "t2a" && isI2v(w) === (kind === "i2v"))).sort((a, b) => rank(a) - rank(b)));
  const GLABEL = $derived<Record<string, string>>({ runpod: ct(S.lang, "rp_grp_runpod"), paid: S.lang === "el" ? "Επί πληρωμής (ανά αποτέλεσμα, θέλει API key)" : "Paid (per result, needs an API key)", open: ct(S.lang, "rp_grp_pc") });
  let groups = $derived(ORDER.map((id) => ({ id, label: GLABEL[id], items: list.filter((w) => grpOf(w) === id) })).filter((g) => g.items.length));
  let wf = $derived(list.find((w) => w.meta.id === wfId) ?? list[0]);
  let slots = $derived((wf?.meta.inputs ?? []).filter((i) => i.type === "image"));
  let name = (w: { meta: { name_el: string; name_en: string } }) => (S.lang === "el" ? w.meta.name_el : w.meta.name_en);
  let label = (i: InputDef) => (S.lang === "el" ? i.label_el : i.label_en) || i.key;
  let fields = $derived((wf?.meta.inputs ?? []).filter((i) => i.key !== "prompt" && i.type !== "image"));

  $effect(() => {
    // keep the selected mode inside the current tab (Text → Video / Image → Video)
    if (list.length && !list.some((w) => w.meta.id === wfId)) {
      // the same mode on the other side (open-source <-> its RunPod twin) keeps the user's choice
      const same = [S.effective(wfId, where)?.meta.id, wfId.replace(/^runpod-/, "")].find((id) => list.some((w) => w.meta.id === id));
      wfId = same ?? list[0].meta.id;
    }
  });

  $effect(() => {
    // apply a draft sent from the Library ("reuse")
    if (S.draft) {
      prompt = S.draft.prompt;
      const dw = S.workflows.find((w) => w.meta.id === S.draft!.workflow);
      if (dw) { kind = dw.meta.kind === "t2a" ? "t2a" : dw.meta.kind === "t2i" ? "t2i" : isI2v(dw) ? "i2v" : "t2v"; wfId = dw.meta.id; }
      S.draft = null;
    }
  });

  function val(i: InputDef) {
    const v = values[wfId]?.[i.key];
    return v ?? (i.default === "__RANDOM_INT__" ? "" : i.default ?? "");
  }
  function setVal(i: InputDef, v: unknown) {
    values[wfId] = { ...(values[wfId] ?? {}), [i.key]: v };
  }
  function collect(): Record<string, unknown> {
    const out: Record<string, unknown> = {};
    for (const i of fields) {
      const v = val(i);
      out[i.key] = i.type === "int" || i.type === "float" ? Number(v) : v;
    }
    return out;
  }

  const PRESETS = [
    { l: "16:9", w: 832, h: 480 }, { l: "9:16", w: 480, h: 832 }, { l: "1:1", w: 640, h: 640 }, { l: "720p", w: 1280, h: 720 },
  ];
  const num = (k: string) => Number(values[wfId]?.[k] ?? fields.find((f) => f.key === k)?.default ?? 0);
  let hasSize = $derived(fields.some((f) => f.key === "width") && fields.some((f) => f.key === "height"));
  let seconds = $derived(fields.some((f) => f.key === "length") && fields.some((f) => f.key === "fps") && num("fps") > 0 ? (num("length") - 1) / num("fps") : 0);
  function preset(w: number, h: number) {
    values[wfId] = { ...(values[wfId] ?? {}), width: w, height: h };
  }
  // Typical price of one result (RunPod and paid API modes). Open-source modes on this PC are free.
  let estimate = $derived(wf?.meta.provider ? (wf.meta.est_usd ?? 0) : 0);

  // Downscale to <=1280px JPEG so the job stays small; match the video size to the start image's shape.
  async function pickFile(file: File | null | undefined, key: string) {
    if (!file || !file.type.startsWith("image/")) return;
    try {
      const bmp = await createImageBitmap(file);
      const k = Math.min(1, 1280 / Math.max(bmp.width, bmp.height));
      const c = document.createElement("canvas");
      c.width = Math.round(bmp.width * k); c.height = Math.round(bmp.height * k);
      c.getContext("2d")!.drawImage(bmp, 0, 0, c.width, c.height);
      const url = c.toDataURL("image/jpeg", 0.92);
      imgs[key] = { b64: url.split(",")[1], url };
      kind = "i2v";
      if (key === "image") {
        const r = bmp.width / bmp.height;
        const [w, h] = r > 1.2 ? [832, 480] : r < 0.83 ? [480, 832] : [640, 640];
        for (const x of S.workflows.filter((x) => isI2v(x))) values[x.meta.id] = { ...(values[x.meta.id] ?? {}), width: w, height: h };
        S.notify(S.t("img_ratio"));
      }
    } catch (e) { S.notify(String(e), true); }
  }
  function clearImage(key: string) { delete imgs[key]; }
  const missingImg = (im: Record<string, Img>) => slots.find((i) => i.required && !im[i.key]);

  function addScene() {
    if (!prompt.trim()) return S.notify(S.t("empty_prompt"), true);
    if (missingImg(imgs)) return S.notify(S.t("img_first"), true);
    scenes.push({ prompt: prompt.trim(), imgs: { ...imgs } });
    prompt = ""; before = null; imgs = {};
    S.notify(S.t("scene_added"));
  }

  async function improve() {
    if (!prompt.trim()) return S.notify(S.t("empty_prompt"), true);
    busy = true;
    const r = await S.run(() => api.enhance(prompt, wf ? name(wf) : ""));
    busy = false;
    if (r) {
      before = prompt; prompt = r.text;
      if (r.tokens) S.notify(`${r.tokens} ${S.t("tokens_used")} · ${r.provider}`);
    }
  }

  async function submit() {
    const KEY_OF: Record<string, string> = { runpod: "runpod_api_key", fal: "fal_key", replicate: "replicate_token", openai: "openai_api_key", gemini: "gemini_api_key" };
    const prov = wf?.meta.provider;
    if (prov === "runpod" && !S.runpodReady) { S.notify(ct(S.lang, "rp_need"), true); S.page = "settings"; return; }
    if (prov && KEY_OF[prov] && !S.secrets[KEY_OF[prov]]) { S.notify(S.t("need_key"), true); S.page = "settings"; return; }
    // Items to queue: image modes use the scene list (+ the form if filled); text modes use one prompt per line.
    let items: { prompt: string; imgs: Record<string, Img> }[];
    if (kind === "i2v") {
      items = [...scenes];
      if (prompt.trim() || Object.keys(imgs).length) {
        if (!prompt.trim()) return S.notify(S.t("empty_prompt"), true);
        if (missingImg(imgs)) return S.notify(S.t("img_first"), true);
        items.push({ prompt: prompt.trim(), imgs: { ...imgs } });
      }
    } else {
      items = (batch ? prompt.split("\n") : [prompt]).map((l) => l.trim()).filter(Boolean).map((l) => ({ prompt: l, imgs: {} }));
    }
    if (!items.length) return S.notify(S.t("empty_prompt"), true);
    await S.set("last_workflow", wfId);
    const base = collect();
    if (runOn) base.provider = runOn;
    let n = 0;
    for (const [i, it] of items.entries()) {
      const p: Record<string, unknown> = { ...base };
      for (const [k, v] of Object.entries(it.imgs)) p[k] = v.b64;
      if (items.length > 1) p.scene = i + 1; // file names get s01_, s02_… so the order is clear
      const j = await S.run(() => api.enqueue(S.projectId, wfId, it.prompt, p));
      if (j) { S.upsert(j); n++; }
    }
    S.notify(`${n} ${S.t("queued_n")}`);
    prompt = ""; before = null; imgs = {}; scenes = [];
  }

  // Three kinds of result; video also has "from a picture". Labels reuse the translated "A → B" tab names (the part after the arrow).
  const MEDIA = [
    { id: "video", kind: "t2v" as const, label: "tab_t2v" as const, icon: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M10 9.5v5l4.5-2.5z"/>' },
    { id: "image", kind: "t2i" as const, label: "tab_t2i" as const, icon: '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M4 18l5-5 4 4 3-3 4 4"/>' },
    { id: "audio", kind: "t2a" as const, label: "tab_t2a" as const, icon: '<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>' },
  ];
  const media = $derived(extra ? extra : kind === "t2i" ? "image" : kind === "t2a" ? "audio" : "video");
  const EXTRA = [
    { id: "voice" as const, el: "Φωνή", en: "Voice", icon: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0014 0M12 18v3"/>' },
    { id: "tools" as const, el: "Εργαλεία", en: "Tools", icon: '<path d="M14 6l4 4M4 20l9-9 4 4-9 9H4z"/><path d="M15 3l1 2 2 1-2 1-1 2-1-2-2-1 2-1z"/>' },
  ];
  const tail = (s: string) => (s.includes("→") ? s.split("→").pop()!.trim() : s);
</script>

<div class="page form">
  <h1>{S.t("nav_generate")}</h1>
  <p class="dim small">{S.t("saving_to")}: {S.project?.dir}</p>

  <div class="card">
    <div class="media" role="tablist">
      {#each MEDIA as m}
        <button role="tab" aria-selected={media === m.id} class="mbtn" class:on={media === m.id} onclick={() => { extra = ""; kind = m.kind === "t2v" && kind === "i2v" ? "i2v" : m.kind; }}>
          <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html m.icon}</svg>
          <span>{tail(S.t(m.label))}</span>
        </button>
      {/each}
      {#each EXTRA as x}
        <button role="tab" aria-selected={extra === x.id} class="mbtn" class:on={extra === x.id} onclick={() => (extra = x.id)}>
          <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html x.icon}</svg>
          <span>{S.lang === "el" ? x.el : x.en}</span>
        </button>
      {/each}
    </div>
    {#if extra}
      {#key extra}<Chain kinds={extra === "voice" ? ["tts", "vc"] : ["i2i", "v2v", "lipsync", "subs", "i2m"]} onclose={() => {}} />{/key}
    {:else}
    {#if media === "video"}
      <div class="from row small">
        <button class="seg" class:on={kind === "t2v"} onclick={() => (kind = "t2v")}>{S.t("tab_t2v")}</button>
        <button class="seg" class:on={kind === "i2v"} onclick={() => (kind = "i2v")}>{S.t("tab_i2v")}</button>
      </div>
    {/if}

    {#if kind === "i2v"}
      <div class="field">
        <div class="small dim" style="margin-bottom:4px">
          {#if slots.length > 1}{S.t("img_need_many").replace("{n}", String(slots.length))}{:else}{S.t("img_need")} 1 {S.t("img_need_n")}{/if}
        </div>
        <div class="grid2">
          {#each slots as sl (sl.key)}
            <div>
              <label for={"imgpick_" + sl.key}>{label(sl)}{sl.required ? " *" : ""}</label>
              <div class="drop {dragging === sl.key ? 'over' : ''}" role="button" tabindex="0"
                ondragover={(e) => { e.preventDefault(); dragging = sl.key; }}
                ondragleave={() => (dragging = "")}
                ondrop={(e) => { e.preventDefault(); dragging = ""; pickFile(e.dataTransfer?.files?.[0], sl.key); }}
                onclick={() => document.getElementById("imgpick_" + sl.key)?.click()}
                onkeydown={(e) => e.key === "Enter" && document.getElementById("imgpick_" + sl.key)?.click()}>
                {#if imgs[sl.key]}<img src={imgs[sl.key].url} alt="" />{:else}<span class="dim">🖼 {S.t("img_pick")}</span>{/if}
              </div>
              <input id={"imgpick_" + sl.key} type="file" accept="image/*" hidden onchange={(e) => { pickFile(e.currentTarget.files?.[0], sl.key); e.currentTarget.value = ""; }} />
              {#if imgs[sl.key]}
                <div class="row small" style="margin-top:4px">
                  <button class="ghost" onclick={() => document.getElementById("imgpick_" + sl.key)?.click()}>{S.t("img_change")}</button>
                  <button class="ghost" onclick={() => clearImage(sl.key)}>{S.t("img_remove")}</button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
        <div class="small dim" style="margin-top:4px">{S.t("img_hint")}</div>
      </div>
    {/if}

    <div class="field">
      <label for="prompt">{S.t("prompt")}</label>
      <textarea id="prompt" bind:value={prompt} placeholder={S.t("prompt_ph")}></textarea>
      <div class="row wrap" style="margin-top:8px">
        <button onclick={improve} disabled={busy}>✨ {busy ? S.t("enhancing") : S.t("enhance")}</button>
        {#if before !== null}<button class="ghost" onclick={() => { prompt = before!; before = null; }}>{S.t("undo")}</button>{/if}
        {#if kind !== "i2v"}<label class="row small dim" style="margin-left:auto"><input type="checkbox" bind:checked={batch} />{S.t("batch")}</label>{/if}
        {#if kind === "i2v"}<button style="margin-left:auto" onclick={addScene}>＋ {S.t("scene_add")}</button>{/if}
      </div>
    </div>

    <div class="field">
      <label for="mode">{S.t("mode")}</label>
      {#if list.length}
        <select id="mode" bind:value={wfId}>
          {#if groups.length > 1}
            {#each groups as g}
              <optgroup label={g.label}>
                {#each g.items as w}<option value={w.meta.id}>{name(w)}</option>{/each}
              </optgroup>
            {/each}
          {:else}
            {#each list as w}<option value={w.meta.id}>{name(w)}</option>{/each}
          {/if}
        </select>
        {#if wf}<div class="small dim" style="margin-top:4px">{S.lang === "el" ? wf.meta.desc_el : wf.meta.desc_en}{wf.meta.vram_gb ? ` · VRAM ≥ ${wf.meta.vram_gb} GB` : ""}</div>
          {#if wf.meta.good_en}<div class="small" style="margin-top:2px">★ {S.lang === "el" ? wf.meta.good_el : wf.meta.good_en}</div>{/if}{/if}
      {:else}
        <div class="msg err">{S.t("no_i2v")}</div>
      {/if}
      <div class="row small" style="margin-top:8px">
        <label class="dim" for="runon">{S.t("run_on")}</label>
        <select id="runon" bind:value={runOn} style="width:auto">
          <option value="">{S.t("run_default")} ({S.provider === "local" ? S.t("run_pc") : "RunPod"})</option>
          <option value="local">{S.t("run_pc")}</option>
          <option value="runpod">RunPod</option>
        </select>
        {#if wf?.meta.provider}<span class="dim">{wf.meta.provider}</span>{/if}
        {#if estimate > 0}<b>≈ ${estimate.toFixed(3)} / {S.t("per_item")}</b>{/if}
      </div>
      {#if wf?.meta.provider === "runpod" && S.rp?.state === "asleep"}<div class="small dim">{ct(S.lang, "rp_wake_hint")}</div>{/if}
    </div>

    {#if kind === "i2v" && scenes.length}
      <div class="field">
        <div class="small dim">{S.t("scenes_title")} ({scenes.length}) — {S.t("scenes_help")}</div>
        {#each scenes as sc, i}
          <div class="row" style="padding:4px 0">
            <b>{i + 1}.</b>
            {#each Object.values(sc.imgs) as im}<img src={im.url} alt="" style="height:36px;border-radius:4px" />{/each}
            <span class="grow small">{sc.prompt}</span>
            <button class="ghost small" onclick={() => scenes.splice(i, 1)}>{S.t("remove")}</button>
          </div>
        {/each}
      </div>
    {/if}

    {#if hasSize}
      <div class="row wrap small" style="margin-bottom:10px">
        <span class="dim">{S.t("presets")}:</span>
        {#each PRESETS as p}<button class="ghost" onclick={() => preset(p.w, p.h)}>{p.l} · {p.w}×{p.h}</button>{/each}
        {#if seconds > 0}<span class="dim" style="margin-left:auto">{S.t("duration")} ≈ {seconds.toFixed(1)} s</span>{/if}
      </div>
    {/if}

    <div class="grid2">
      {#each fields.filter((f) => !f.advanced) as f}
        <div class="field">
          <label for={"f_" + f.key}>{label(f)}</label>
          {#if f.options?.length}
            <select id={"f_" + f.key} value={val(f) as string} onchange={(e) => setVal(f, e.currentTarget.value)}>
              {#each f.options as o}<option value={o}>{o}</option>{/each}
            </select>
          {:else}
            <input id={"f_" + f.key} type={f.type === "int" || f.type === "float" ? "number" : "text"}
              min={f.min} max={f.max} step={f.step} value={val(f) as string}
              oninput={(e) => setVal(f, e.currentTarget.value)} />
          {/if}
        </div>
      {/each}
    </div>

    {#if fields.some((f) => f.advanced)}
      <button class="ghost small" onclick={() => (showAdv = !showAdv)}>{showAdv ? "▾" : "▸"} {S.t("advanced")}</button>
      {#if showAdv}
        {#each fields.filter((f) => f.advanced) as f}
          <div class="field" style="margin-top:8px">
            <label for={"f_" + f.key}>{label(f)}</label>
            {#if f.type === "text"}
              <textarea id={"f_" + f.key} style="min-height:60px" value={val(f) as string} oninput={(e) => setVal(f, e.currentTarget.value)}></textarea>
            {:else}
              <input id={"f_" + f.key} value={val(f) as string} oninput={(e) => setVal(f, e.currentTarget.value)} />
            {/if}
          </div>
        {/each}
      {/if}
    {/if}

    <div class="row submitbar">
      <div class="grow small dim">
        {#if estimate > 0}{S.t("est_cost")}: <b>${estimate.toFixed(3)}</b> {S.t("est_cost_note")}{:else}{S.t("no_price")}{/if}
      </div>
      <button class="primary big" onclick={submit} disabled={!wf}>{batch || scenes.length ? S.t("add_queue_n") : S.t("add_queue")}</button>
    </div>
    {/if}
  </div>

  <h2>{S.t("now_running")}</h2>
  {#if S.pending.length}
    {#each S.pending.slice().reverse() as j (j.id)}<JobCard job={j} />{/each}
  {:else}
    <p class="dim">{S.t("nothing_running")}</p>
  {/if}
</div>

<style>
  .media { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; margin-bottom: 14px; }
  .mbtn { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 14px 10px; background: var(--panel2); border-radius: 10px; color: var(--dim); font-family: var(--font-head); font-size: 16px; }
  .mbtn.on { color: var(--text); border-color: var(--accent); background: var(--sel); }
  .mbtn.on svg { color: var(--accent); }
  .from { gap: 4px; margin: -4px 0 16px; }
  .seg { background: transparent; border-radius: 99px; padding: 4px 12px; color: var(--dim); }
  .seg.on { color: var(--text); border-color: var(--accent); background: var(--sel); }
  .submitbar { position: sticky; bottom: -18px; margin: 14px -18px -18px; padding: 12px 18px; background: var(--panel); border-top: 1px solid var(--line); border-radius: 0 0 var(--r-sheet) var(--r-sheet); }
  .big { padding: 10px 26px; font-size: 15px; }
  #prompt { min-height: 140px; font-size: 15px; line-height: 1.5; }
</style>
