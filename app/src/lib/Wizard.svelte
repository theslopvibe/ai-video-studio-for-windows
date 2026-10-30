<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { S } from "$lib/store.svelte";
  import { api, type Hardware, type LocalStatus, type SetupProgress } from "$lib/api";
  import { wt, type WKey } from "$lib/wizard";

  const t = (k: WKey) => wt(S.lang, k);
  const steps: WKey[] = ["s1", "s2", "s3", "s4", "s5"];

  let step = $state(0);
  let where = $state<"pc" | "cloud" | "both">("pc");
  let hw = $state<Hardware | null>(null);
  let local = $state<LocalStatus | null>(null);
  let busy = $state(false);
  let prog = $state<SetupProgress | null>(null);
  let err = $state("");
  let ok = $state("");
  let folder = $state("");
  let key = $state("");
  let cprov = $state<"vast" | "fal" | "replicate">("vast");
  const secretOf = $derived(cprov === "vast" ? "vast_api_key" : cprov === "fal" ? "fal_key" : "replicate_token");
  let picked = $state<string[]>([]);
  let prompt = $state(t("f_sample"));

  const needPc = $derived(where !== "cloud");
  const needCloud = $derived(where !== "pc");
  const vramGb = $derived(hw?.gpus[0] ? hw.gpus[0].vram_mb / 1024 : 0);
  const canPc = $derived(!!hw && hw.support !== "none");
  const packs = $derived(S.boot?.models.filter((m) => m.kind === "video" || m.kind === "image") ?? []);

  function fit(vram: number): "fits" | "tight" | "toobig" {
    if (vram <= vramGb + 0.5) return "fits";
    return vram <= vramGb * 1.6 ? "tight" : "toobig";
  }

  onMount(() => {
    const un = listen<SetupProgress>("setup-progress", (e) => (prog = e.payload));
    api.hardwareScan().then((h) => {
      hw = h;
      where = h.support !== "none" ? "pc" : "cloud";
      preselect();
    }).catch((e) => (err = String(e)));
    api.localStatus().then((s) => (local = s)).catch(() => {});
    return () => { un.then((f) => f()); };
  });

  function preselect() {
    // the best video model that fits, otherwise the smallest one that is merely tight
    const vids = packs.filter((m) => m.kind === "video" && fit(m.vram_gb) !== "toobig").sort((a, b) => b.vram_gb - a.vram_gb);
    const fitting = vids.filter((m) => fit(m.vram_gb) === "fits");
    const choice = fitting[0] ?? vids.sort((a, b) => a.vram_gb - b.vram_gb)[0];
    picked = choice ? [choice.id] : [];
  }

  const label = (s: string) => ({ tools: t("p_tools"), comfy: t("p_comfy"), extract: t("p_extract"), nodes: t("p_nodes"), models: t("p_models") } as Record<string, string>)[s] ?? s;

  async function run(f: () => Promise<void>) {
    busy = true; err = ""; ok = ""; prog = null;
    try { await f(); } catch (e) { err = String(e); } finally { busy = false; }
  }

  const install = () => run(async () => {
    await api.localInstall();
    local = await api.localStatus();
    ok = t("testing");
    await api.localStart();
    ok = t("works");
    local = await api.localStatus();
  });

  const useFolder = () => run(async () => {
    await S.set("local_comfy_dir", folder.trim());
    local = await api.localStatus();
    if (!local.installed) throw new Error(S.lang === "el" ? "Δεν βρέθηκαν τα python_embeded και ComfyUI σε αυτόν τον φάκελο." : "python_embeded and ComfyUI were not found in that folder.");
    await api.localStart();
    ok = t("works");
  });

  const saveKey = () => run(async () => {
    S.secrets = await api.setSecret(secretOf, key.trim());
    key = "";
    if (cprov === "vast") {
      const r = await api.testVast();
      if (!r.ok) throw new Error(r.message);
    } else {
      await api.hostedTest(cprov);
    }
    ok = t("key_ok");
  });

  const downloadModels = () => run(async () => {
    await api.localInstallModels(picked);
    local = await api.localStatus();
    ok = t("have");
  });

  async function go(n: number) { err = ""; ok = ""; prog = null; step = n; }

  async function finish(generate: boolean) {
    await S.set("provider", where === "cloud" ? "vast" : "local");
    await S.set("setup_done", "1");
    S.wizard = false;
    if (generate) {
      const hostedOnly = !needPc && cprov !== "vast";
      const wf = S.workflows.filter((w) => w.meta.kind === "t2v" && (hostedOnly ? w.meta.provider === cprov : !w.meta.provider)).sort((a, b) => (a.meta.priority ?? 100) - (b.meta.priority ?? 100));
      const pack = picked[0];
      const w = (needPc && pack ? wf.find((x) => x.meta.pack === pack) : undefined) ?? wf[0];
      if (w) {
        await S.run(async () => { await api.enqueue(S.projectId, w.meta.id, prompt, {}); });
        S.page = "queue";
      }
    }
  }

  const canNext = $derived(
    step === 1 ? !!hw && (where === "cloud" || canPc)
    : step === 2 ? (!needPc || !!local?.installed) && (!needCloud || !!S.secrets[secretOf])
    : true,
  );
</script>

<div class="wiz">
  <aside>
    <div class="brand">AI Video Studio For Windows</div>
    <ol>
      {#each steps as s, i}
        <li class:on={i === step} class:done={i < step}>
          <span class="dot">{i < step ? "✓" : i + 1}</span>{t(s)}
        </li>
      {/each}
    </ol>
    <div class="grow"></div>
    <button class="ghost small" onclick={() => finish(false)}>{t("skip")}</button>
  </aside>

  <section>
    {#if step === 0}
      <h1>{t("title")}</h1>
      <p class="lead">{t("lead")}</p>
      <p class="dim">{t("free")}</p>
      <div class="row">
        <button class:primary={S.lang === "en"} onclick={() => { S.lang = "en"; S.set("lang", "en"); }}>English</button>
        <button class:primary={S.lang === "el"} onclick={() => { S.lang = "el"; S.set("lang", "el"); }}>Ελληνικά</button>
      </div>
      <div class="nav"><button class="primary big" onclick={() => go(1)}>{t("start")} →</button></div>

    {:else if step === 1}
      <h1>{t("q_where")}</h1>
      <div class="card hw">
        {#if !hw}
          <span class="dim">{t("scanning")}</span>
        {:else if hw.gpus.length}
          <div class="small dim">{t("found")}</div>
          <div class="gpu">{hw.gpus[0].name}{hw.gpus[0].vram_mb ? ` · ${(hw.gpus[0].vram_mb / 1024).toFixed(0)} GB` : ""}</div>
          <div class="small dim">{t("ram")} {hw.ram_gb} GB · {hw.disk_free_gb} GB {t("disk")} ({hw.disk_path})</div>
        {:else}
          <div>{t("no_gpu")}</div>
        {/if}
        {#if hw && (hw.support === "amd" || hw.support === "intel")}<div class="msg warn">{t("exp_gpu")}</div>{/if}
      </div>
      <div class="choices">
        <button class="choice" class:sel={where === "pc"} disabled={!canPc} onclick={() => (where = "pc")}>
          <span class="ttl"><b>{t("pc")}</b>{#if canPc}<span class="pill ok">{t("recommended")}</span>{/if}</span>
          <span class="dim">{t("pc_d")}</span>
        </button>
        <button class="choice" class:sel={where === "cloud"} onclick={() => (where = "cloud")}>
          <span class="ttl"><b>{t("cloud")}</b>{#if hw && !canPc}<span class="pill ok">{t("recommended")}</span>{/if}</span>
          <span class="dim">{t("cloud_d")}</span>
        </button>
        <button class="choice" class:sel={where === "both"} disabled={!canPc} onclick={() => (where = "both")}>
          <b>{t("both")}</b><span class="dim">{t("both_d")}</span>
        </button>
      </div>
      <div class="nav">
        <button onclick={() => go(0)}>← {t("back")}</button>
        <button class="primary" disabled={!canNext} onclick={() => go(2)}>{t("next")} →</button>
      </div>

    {:else if step === 2}
      {#if needPc}
        <h1>{t("inst_pc")}</h1>
        <p class="dim">{t("inst_pc_d")}</p>
        {#if local?.installed}
          <div class="msg ok">{t("installed")} <span class="dim small">{local.dir}</span></div>
        {/if}
        {#if !local?.installed}
          <div class="row">
            <button class="primary" disabled={busy} onclick={install}>{t("install_btn")}</button>
            {#if busy}<button onclick={() => api.setupCancel()}>{t("cancel")}</button>{/if}
          </div>
          <details style="margin-top:14px">
            <summary>{t("have_comfy")}</summary>
            <p class="small dim">{t("have_comfy_d")}</p>
            <div class="row"><input bind:value={folder} placeholder="C:\ComfyUI_windows_portable" /><button disabled={busy || !folder.trim()} onclick={useFolder}>{t("use_folder")}</button></div>
          </details>
        {/if}
      {/if}
      {#if needCloud}
        <h1 style={needPc ? "margin-top:28px" : ""}>{t("inst_cloud")}</h1>
        <div class="field"><label for="prov">{t("provider")}</label>
          <select id="prov" bind:value={cprov} onchange={() => { ok = ""; err = ""; }}><option value="vast">Vast.ai (rent a GPU)</option><option value="fal">fal.ai (pay per result)</option><option value="replicate">Replicate (pay per result)</option><option disabled>RunPod ({t("soon")})</option></select>
        </div>
        <p class="dim">{t(cprov === "vast" ? "vast_how" : cprov === "fal" ? "fal_how" : "rep_how")}</p>
        <div class="row"><input type="password" bind:value={key} placeholder={t("api_key")} /><button class="primary" disabled={busy || !key.trim()} onclick={saveKey}>{t("save_test")}</button></div>
        {#if S.secrets[secretOf] && !ok}<div class="msg ok">{t("key_ok")}</div>{/if}
      {/if}

      {#if prog && busy}
        <div class="progress"><div style="width:{Math.max(2, prog.pct)}%"></div></div>
        <div class="small dim">{label(prog.step)} · {prog.detail}</div>
      {:else if busy}<div class="small dim">…</div>{/if}
      {#if ok}<div class="msg ok">{ok}</div>{/if}
      {#if err}<div class="msg err">{err}</div>{/if}
      <div class="nav">
        <button onclick={() => go(1)}>← {t("back")}</button>
        <button class="primary" disabled={!canNext || busy} onclick={() => go(3)}>{t("next")} →</button>
      </div>

    {:else if step === 3}
      <h1>{t("m_title")}</h1>
      {#if needPc}
        <p class="dim">{t("m_intro")}</p>
        {#each packs as m}
          {@const f = fit(m.vram_gb)}
          {@const have = m.files.every((x) => local?.models.some((y) => y.endsWith(x.url.split("/").pop() ?? "?")))}
          <label class="pack" class:off={f === "toobig"}>
            <input type="checkbox" checked={picked.includes(m.id)} disabled={f === "toobig" || busy}
              onchange={(e) => (picked = e.currentTarget.checked ? [...picked, m.id] : picked.filter((x) => x !== m.id))} />
            <div>
              <b>{m.name}</b>
              <span class="pill {f === 'fits' ? 'ok' : f === 'tight' ? 'warn' : 'err'}">{t(f === "fits" ? "fits" : f === "tight" ? "tight" : "toobig")}</span>
              {#if have}<span class="pill ok">{t("have")}</span>{/if}
              <div class="small dim">{S.lang === "el" ? m.desc_el : m.desc_en} · {m.size_gb} GB · {m.license}</div>
            </div>
          </label>
        {/each}
        <div class="row" style="margin-top:12px">
          <button class="primary" disabled={busy || !picked.length} onclick={downloadModels}>{t("download_sel")} ({packs.filter((m) => picked.includes(m.id)).reduce((a, m) => a + m.size_gb, 0).toFixed(0)} GB)</button>
          {#if busy}<button onclick={() => api.setupCancel()}>{t("cancel")}</button>{/if}
        </div>
      {:else}
        <p class="dim">{t("m_cloud")}</p>
      {/if}
      {#if prog && busy}
        <div class="progress"><div style="width:{Math.max(2, prog.pct)}%"></div></div>
        <div class="small dim">{label(prog.step)} · {prog.detail}</div>
      {/if}
      {#if ok}<div class="msg ok">{ok}</div>{/if}
      {#if err}<div class="msg err">{err}</div>{/if}
      <div class="nav">
        <button onclick={() => go(2)}>← {t("back")}</button>
        <button class="primary" disabled={busy} onclick={() => go(4)}>{t("next")} →</button>
      </div>

    {:else}
      <h1>{t("f_title")}</h1>
      <p class="dim">{t("f_intro")}</p>
      <textarea rows="4" bind:value={prompt}></textarea>
      <p class="small dim">{t("f_prompt_note")}</p>
      <div class="nav">
        <button onclick={() => go(3)}>← {t("back")}</button>
        <button class="ghost" onclick={() => finish(false)}>{t("f_skip")}</button>
        <button class="primary big" disabled={!prompt.trim()} onclick={() => finish(true)}>✦ {t("f_go")}</button>
      </div>
    {/if}
  </section>
</div>

<style>
  .wiz { display: grid; grid-template-columns: 250px 1fr; height: 100vh; }
  aside { background: var(--panel); border-right: 1px solid var(--line); padding: 24px 18px; display: flex; flex-direction: column; gap: 22px; }
  .brand { font-weight: 700; font-size: 16px; }
  ol { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  li { display: flex; align-items: center; gap: 12px; padding: 9px 10px; border-radius: 10px; color: var(--dim); }
  li.on { background: var(--sel); color: var(--text); font-weight: 600; }
  li.done { color: var(--text); }
  .dot { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line); display: grid; place-items: center; font-size: 12px; flex: none; }
  li.on .dot { background: var(--accent2); border-color: var(--accent2); color: #fff; }
  li.done .dot { border-color: var(--ok); color: var(--ok); }
  .grow { flex: 1; }
  section { padding: 44px 56px; overflow-y: auto; max-width: 900px; }
  h1 { font-size: 26px; margin: 0 0 12px; }
  .lead { font-size: 16px; line-height: 1.55; max-width: 62ch; }
  .row { display: flex; gap: 10px; align-items: center; margin-top: 14px; }
  .nav { display: flex; gap: 10px; margin-top: 30px; align-items: center; }
  .nav .primary { margin-left: auto; }
  .big { padding: 12px 22px; font-size: 15px; }
  .hw .gpu { font-size: 18px; font-weight: 600; margin: 2px 0; }
  .choices { display: grid; gap: 10px; margin-top: 8px; }
  .choice { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; text-align: left; padding: 14px 16px; border-radius: 12px; }
  .ttl { display: flex; gap: 8px; align-items: center; }
  .choice.sel { border-color: var(--accent); background: var(--sel); }
  .choice:disabled { opacity: .45; cursor: not-allowed; }
  .pack { display: flex; gap: 12px; align-items: flex-start; padding: 12px; border: 1px solid var(--line); border-radius: 12px; margin-bottom: 8px; cursor: pointer; }
  .pack.off { opacity: .5; }
  .progress { height: 8px; background: var(--panel2); border-radius: 99px; overflow: hidden; margin: 18px 0 6px; }
  .progress div { height: 100%; background: var(--accent); transition: width .3s; }
  details summary { cursor: pointer; color: var(--dim); }
</style>
