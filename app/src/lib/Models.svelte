<script lang="ts">
  import { api, type HfHit, type HfRepo } from "./api";
  import Stars from "./Stars.svelte";
  import { open as pickDialog } from "@tauri-apps/plugin-dialog";
  import { S } from "./store.svelte";
  import { ask as askDialog } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let packs = $derived(S.boot?.models ?? []);

  // ---- "Add model" from HuggingFace
  const DIRS = ["diffusion_models", "checkpoints", "vae", "text_encoders", "loras", "clip_vision", "controlnet", "upscale_models", "unet"] as const;
  let adding = $state(false);
  let q = $state("");
  let videoOnly = $state(true);
  let looking = $state(false);
  let hits = $state<HfHit[] | null>(null);
  let repo = $state<HfRepo | null>(null);
  let picked = $state<Record<string, boolean>>({});
  let dirs = $state<Record<string, string>>({});
  let mname = $state("");
  let mkind = $state("video");
  let mvram = $state(24);
  let fileFilter = $state("");
  let shownFiles = $derived(repo ? repo.files.filter((f) => f.path.toLowerCase().includes(fileFilter.toLowerCase())) : []);
  const MODE: Record<string, string> = { t2v: "mode_t2v", i2v: "mode_i2v", t2i: "mode_t2i", i2i: "mode_i2i", flf: "mode_flf" };

  async function lookup(query = q, fromHit = false) {
    if (!query.trim()) return;
    if (!fromHit) q = query; looking = true; repo = null;
    const r = await S.run(() => api.hfLookup(query, videoOnly));
    looking = false;
    if (!r) return;
    if (r.kind === "hits") hits = r.data;
    else {
      if (!fromHit) hits = null;
      repo = r.data; picked = {}; dirs = {};
      for (const f of r.data.files) dirs[f.path] = f.dir;
      // what the app worked out: type, VRAM, and the best variant of each file pre-selected
      const ins = r.data.insight;
      mname = r.data.id.split("/")[1] ?? r.data.id;
      mkind = ins.kind === "image" ? "image" : "video";
      if (ins.vram_gb) mvram = ins.vram_gb;
      fileFilter = "";
      if (!ins.too_many) for (const f of r.data.files) if (f.recommended) picked[f.path] = true;
    }
  }
  const gb = (n: number) => (n / 1e9).toFixed(1);
  let pickedGb = $derived(repo ? repo.files.filter((f) => picked[f.path]).reduce((a, f) => a + f.size, 0) / 1e9 : 0);
  async function addModel() {
    if (!repo) return;
    const files = repo.files.filter((f) => picked[f.path]).map((f) => [f.path, dirs[f.path], f.size] as [string, string, number]);
    const r = await S.run(() => api.customModelAdd(repo!.id, mname, mkind, mvram, repo!.license, files, repo!.insight));
    if (r) {
      S.boot!.models = r;
      sel = [...sel, "hf-" + repo.id.toLowerCase().replace("/", "--")];
      S.notify(S.t("hf_added"));
      adding = false; repo = null; hits = null; q = "";
    }
  }
  async function removeModel(id: string) {
    const r = await S.run(() => api.customModelRemove(id));
    if (r) { S.boot!.models = r; sel = sel.filter((x) => x !== id); }
  }
  async function importWorkflow() {
    const f = await pickDialog({ multiple: false, filters: [{ name: "Workflow", extensions: ["json"] }] });
    if (typeof f !== "string") return;
    const r = await S.run(() => api.workflowImport(f));
    if (r) { S.workflows = r[0]; S.workflowErrors = r[1]; S.notify(S.t("wf_imported")); }
  }
  let sel = $state<string[]>((S.settings.models_selected ?? "").split(",").filter(Boolean));
  let script = $state("");
  let busy = $state(false);
  let note = $state<{ ok: boolean; text: string } | null>(null);

  let totalGb = $derived(packs.filter((p) => sel.includes(p.id) && !p.in_template).reduce((a, p) => a + p.size_gb, 0));
  const toggle = (id: string) => (sel = sel.includes(id) ? sel.filter((x) => x !== id) : [...sel, id]);
  const kind = (k: string) => (k === "video" ? "video" : k === "image" ? (S.lang === "el" ? "εικόνα" : "image") : k);

  async function preview() { const r = await S.run(() => api.installOnWorkers(sel, false)); if (r) script = r.script; }
  async function install() {
    busy = true; note = null;
    try { const r = await api.installOnWorkers(sel, true); script = r.script; note = { ok: true, text: r.message }; }
    catch (e) { note = { ok: false, text: String(e) }; }
    busy = false;
  }
  let warm = $state<{ state: string; text: string; secs: number } | null>(null);
  onMount(() => {
    const un = listen<{ state: string; text: string; secs: number }>("warmup", (e) => {
      warm = e.payload;
      if (e.payload.state === "ready") S.notify(S.t("warm_ready"));
    });
    return () => { un.then((f) => f()); };
  });
  async function prepare() {
    if (!(await askDialog(S.t("warm_confirm"), { kind: "warning" }))) return;
    warm = { state: "working", text: "", secs: 0 };
    try { await api.warmUp(); } catch (e) { warm = { state: "error", text: String(e), secs: 0 }; }
  }
  const mmss = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  async function copy() { await navigator.clipboard.writeText(script); S.notify(S.t("copied")); }
  async function reload() {
    const r = await S.run(() => api.reloadWorkflows());
    if (r) { S.workflows = r[0]; S.workflowErrors = r[1]; }
  }
</script>

<div class="page">
  <h1>{S.t("models_title")}</h1>
  <p class="dim">{S.t("models_intro")}</p>

  <div class="card">
    <table>
      <tbody>
        {#each packs as p}
          <tr>
            <td style="width:30px"><input type="checkbox" disabled={p.in_template} checked={p.in_template || sel.includes(p.id)} onchange={() => toggle(p.id)} /></td>
            <td>
              <b>{p.name}</b> <span class="pill">{kind(p.kind)}</span> <span class="pill">{p.license}</span>
              {#if p.in_template}<span class="pill ok">{S.t("in_template")}</span>{/if}
              {#if p.insight}<span class="small"> · {S.t("found_quality")} <Stars n={p.insight.quality} /> · {S.t("found_speed")} <Stars n={p.insight.speed} /></span>{/if}
              {#if p.custom}<span class="pill">{S.t("custom_tag")}</span> <button class="ghost small" onclick={() => removeModel(p.id)}>{S.t("remove")}</button>{/if}
              <div class="small dim">{S.lang === "el" ? p.desc_el : p.desc_en}</div>
            </td>
            <td class="num small dim">{p.size_gb} GB<br />VRAM ≥ {p.vram_gb} GB</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row" style="margin-top:10px">
      <div class="grow small dim">{S.t("total_size")}: <b>{totalGb} GB</b></div>
      <button onclick={preview}>{S.t("install_preview")}</button>
      <button class="primary" onclick={install} disabled={busy}>{busy ? S.t("installing") : S.t("install_workers")}</button>
    </div>
    <p class="small dim" style="margin-top:8px">{S.t("install_help")}</p>
    {#if note}<div class="msg {note.ok ? 'ok' : 'err'}">{note.text}</div>{/if}
    <div class="row" style="margin-top:12px">
      <button class="primary" onclick={prepare} disabled={warm?.state === "working"}>{S.t("warm_btn")}</button>
      {#if warm?.state === "working"}<button onclick={() => api.warmCancel()}>{S.t("warm_stop")}</button>{/if}
      <div class="grow small dim">{S.t("warm_help")}</div>
    </div>
    {#if warm?.state === "working"}<div class="msg">{S.t("warm_working")} {mmss(warm.secs)}{warm.text ? " · " + warm.text : ""}</div>{/if}
    {#if warm?.state === "ready"}<div class="msg ok"><b>{S.t("warm_ready")}</b></div>{/if}
    {#if warm?.state === "error"}<div class="msg err">{warm.text}</div>{/if}
    {#if script}
      <pre>{script}</pre>
      <button onclick={copy}>{S.t("copy")}</button>
    {/if}
  </div>

  <div class="card">
    <div class="row">
      <h2 class="grow" style="margin:0">{S.t("add_model")}</h2>
      <button onclick={() => (adding = !adding)}>{adding ? "▾" : "▸"} {S.t("add_model")}</button>
    </div>
    {#if adding}
      <p class="small dim">{S.t("add_model_help")}</p>
      <div class="row">
        <input class="grow" placeholder={S.t("hf_ph")} bind:value={q} onkeydown={(e) => e.key === "Enter" && lookup()} />
        <button class="primary" onclick={() => lookup()} disabled={looking}>{looking ? "…" : S.t("hf_search")}</button>
      </div>
      <label class="row small dim" style="margin-top:6px"><input type="checkbox" bind:checked={videoOnly} />{S.t("hf_video_only")}</label>

      {#if hits}
        <h3>{S.t("hf_results")}</h3>
        {#if !hits.length}<p class="dim">{S.t("hf_none")}</p>{/if}
        {#each hits as h}
          <div class="row" style="padding:4px 0">
            <button class="ghost grow" style="text-align:left" onclick={() => lookup(h.id, true)}>{h.id}</button>
            <span class="small dim">⬇ {h.downloads.toLocaleString()} · ♥ {h.likes}</span>
          </div>
        {/each}
      {/if}

      {#if repo}
        {#if hits}<button class="ghost small" onclick={() => (repo = null)}>{S.t("hf_back")}</button>{/if}
        <h3>{repo.id} <span class="pill">{repo.license}</span></h3>
        {#if repo.gated}<div class="msg err">{S.t("hf_gated")}</div>{/if}
        {#if repo.diffusers}<div class="msg warn">{S.t("hf_diffusers")}</div>{/if}
        {#if !repo.files.length}<div class="msg err">{S.t("hf_nofiles")}</div>{/if}
        {#if repo.files.length && !repo.gated}
          {@const ins = repo.insight}
          <div class="msg">
            <b>{S.t("found_title")}</b>
            <div class="grid2 small" style="margin-top:6px">
              <div>{S.t("found_type")}: <b>{ins.kind === "image" ? S.t("hf_kind_image") : S.t("hf_kind_video")}</b>{#if ins.modes.length} · {ins.modes.map((m) => S.t((MODE[m] ?? "mode_t2v") as never)).join(", ")}{/if}</div>
              <div>{S.t("found_family")}: <b>{ins.family || S.t("found_unknown")}</b>{#if ins.params_b} · ~{ins.params_b}B{/if}{#if ins.precision} · {ins.precision}{/if}</div>
              <div>{S.t("found_quality")}: <Stars n={ins.quality} /> <span class="dim">({S.t(ins.basis === "family" ? "basis_family" : "basis_size")})</span></div>
              <div>{S.t("found_speed")}: <Stars n={ins.speed} /></div>
              <div>{S.t("found_vram")}: <b>≥ {ins.vram_gb || "?"} GB</b></div>
              <div>{S.t("found_license")}: <b>{repo.license}</b> {ins.commercial === true ? "✔ " + S.t("found_commercial") : ins.commercial === false ? "⚠ " + S.t("found_noncommercial") : ""}</div>
            </div>
            {#if ins.typical.length}
              <div class="small dim" style="margin-top:6px">{S.t("found_typical")}: {ins.typical.map(([k, v]) => S.t(("typ_" + k) as never) + " " + v).join(" · ")}</div>
            {/if}
            <div class="small" style="margin-top:6px">
              {#if ins.workflows.length}✔ {S.t("found_wf_ready")}: {ins.workflows.map((id) => { const w = S.workflows.find((x) => x.meta.id === id); return w ? (S.lang === "el" ? w.meta.name_el : w.meta.name_en) : id; }).join(" · ")}
              {:else}⚠ {S.t("found_wf_needed")}{/if}
            </div>
            {#if ins.too_many}<div class="small" style="margin-top:6px">⚠ {S.t("found_many")}</div>{/if}
          </div>
          <p class="small dim">{S.t("hf_files")}</p>
          <input placeholder={S.t("file_filter")} bind:value={fileFilter} style="margin-bottom:6px" />
          <table>
            <tbody>
              {#each shownFiles as f}
                <tr>
                  <td style="width:30px"><input type="checkbox" bind:checked={picked[f.path]} /></td>
                  <td class="small">{f.path} {#if f.precision}<span class="pill">{f.precision}</span>{/if} {#if f.recommended}<span class="pill ok">{S.t("recommended")}</span>{/if}</td>
                  <td class="num small dim">{gb(f.size)} GB</td>
                  <td>
                    <select bind:value={dirs[f.path]} aria-label={S.t("hf_where")}>
                      {#each DIRS as d}<option value={d}>{S.t(("dir_" + d) as never)}</option>{/each}
                    </select>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
          <div class="grid2" style="margin-top:10px">
            <div class="field"><label for="mn">{S.t("hf_name")}</label><input id="mn" bind:value={mname} /></div>
            <div class="field"><label for="mk">{S.t("hf_kind")}</label>
              <select id="mk" bind:value={mkind}><option value="video">{S.t("hf_kind_video")}</option><option value="image">{S.t("hf_kind_image")}</option></select></div>
            <div class="field"><label for="mv">{S.t("hf_vram")}</label><input id="mv" type="number" min="4" max="80" bind:value={mvram} /></div>
          </div>
          <div class="row">
            <div class="grow small dim">{S.t("total_size")}: <b>{pickedGb.toFixed(1)} GB</b> · {S.t("hf_need_wf")}</div>
            <button class="primary" onclick={addModel} disabled={pickedGb === 0}>{S.t("hf_add")}</button>
          </div>
        {/if}
      {/if}
    {/if}
  </div>

  <h2>{S.t("workflows_title")}</h2>
  <p class="dim small">{S.t("workflows_help")}</p>
  <div class="card">
    {#each S.workflows as w}
      <div class="row" style="padding:4px 0">
        <div class="grow">{S.lang === "el" ? w.meta.name_el : w.meta.name_en} <span class="small dim">· {w.file}</span></div>
        <span class="pill {w.source === 'user' ? 'ok' : ''}">{S.t(w.source === "user" ? "user" : "bundled")}</span>
      </div>
    {/each}
    {#each S.workflowErrors as er}<div class="msg err">{er}</div>{/each}
    <div class="row" style="margin-top:10px">
      <button onclick={() => S.run(() => api.reveal(S.boot!.user_workflows_dir))}>{S.t("open_folder")}</button>
      <button onclick={importWorkflow}>{S.t("wf_import")}</button>
      <button onclick={reload}>{S.t("reload")}</button>
    </div>
  </div>
</div>
