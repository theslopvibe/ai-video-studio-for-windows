<script lang="ts">
  import { open as pickDialog, save as saveDialog, ask as askDialog } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import { S } from "./store.svelte";
  import type { Lang } from "./i18n";
  import { LANGS } from "./translations";
  import type { Theme } from "./store.svelte";
  import { ct } from "./chatStrings";
  import ModelSelect from "./ModelSelect.svelte";
  import { check as checkUpdate } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  let upd = $state("");
  let updBusy = $state(false);
  async function updateApp() {
    const el = S.lang === "el";
    updBusy = true; upd = el ? "Έλεγχος…" : "Checking…";
    try {
      const u = await checkUpdate();
      if (!u) { upd = el ? "Έχεις την τελευταία έκδοση." : "You are up to date."; return; }
      if (!(await askDialog(el ? `Νέα έκδοση ${u.version}. Λήψη και εγκατάσταση τώρα; Τα δεδομένα σου μένουν.` : `Version ${u.version} is available. Download and install now? Your data stays.`, { kind: "info" }))) { upd = ""; return; }
      upd = el ? "Λήψη…" : "Downloading…";
      await u.downloadAndInstall();
      await relaunch();
    } catch (e) { upd = (el ? "Αποτυχία ενημέρωσης: " : "Update failed: ") + String(e); }
    finally { updBusy = false; }
  }

  const provKeys: { name: string; label: string; hosted?: string; help: string }[] = [
    { name: "fal_key", label: "fal.ai", hosted: "fal", help: "fal.ai → Dashboard → Keys" },
    { name: "replicate_token", label: "Replicate", hosted: "replicate", help: "replicate.com → Account → API tokens" },
    { name: "glm_api_key", label: "Z.ai GLM", help: "z.ai → API keys" },
    { name: "deepseek_api_key", label: "DeepSeek", help: "platform.deepseek.com → API keys" },
    { name: "openrouter_api_key", label: "OpenRouter", help: "openrouter.ai → Keys" },
  ];
  let provVals = $state<Record<string, string>>({});
  let provRes = $state<Record<string, { ok: boolean; text: string }>>({});
  async function saveProv(k: { name: string; hosted?: string }) {
    await saveKey(k.name, provVals[k.name] ?? ""); provVals[k.name] = "";
    if (k.hosted && S.secrets[k.name]) {
      try { provRes[k.name] = { ok: true, text: await api.hostedTest(k.hosted) }; } catch (e) { provRes[k.name] = { ok: false, text: String(e) }; }
    }
  }

  let vast = $state("");
  let anth = $state("");
  let geminiKey = $state("");
  let openaiKey = $state("");
  let endpoint = $state(S.settings.endpoint_name ?? "comfyui-json");
  let busy = $state<string>("");
  let res = $state<Record<string, { ok: boolean; text: string }>>({});

  async function saveKey(name: string, v: string) {
    const r = await S.run(() => api.setSecret(name, v));
    if (r) { S.secrets = r; S.notify(S.t("saved")); }
  }
  let provider = $derived(["api", "gemini", "openai"].includes(S.settings.enhance_provider) ? S.settings.enhance_provider : "cc");
  async function testLlm() {
    busy = "llm";
    try { res.llm = { ok: true, text: await api.testLlm() }; } catch (e) { res.llm = { ok: false, text: String(e) }; }
    busy = "";
  }
  async function setupVast() {
    const ok = await askDialog(S.t("setup_confirm"), { title: S.t("setup_btn"), kind: "warning" });
    if (!ok) return;
    busy = "vast";
    try {
      const gpus = (S.settings.gpu_selection ?? "").split(",").filter(Boolean);
      const m = await api.setupEndpoint(gpus);
      res.vast = { ok: true, text: m };
    } catch (e) { res.vast = { ok: false, text: String(e) }; }
    busy = "";
    await test("vast");
  }
  async function test(which: "vast") {
    busy = which;
    try {
      await S.set("endpoint_name", endpoint.trim() || "comfyui-json");
      const r = await api.testVast();
      res.vast = { ok: r.ok && !!r.endpoint?.scales_to_zero, text: r.message };
    } catch (e) { res[which] = { ok: false, text: String(e) }; }
    busy = "";
  }
  async function zero() {
    try { res.vast = { ok: true, text: await api.scaleToZero() }; await test("vast"); }
    catch (e) { res.vast = { ok: false, text: String(e) }; }
  }
  async function chooseDir() {
    const d = await pickDialog({ directory: true });
    if (typeof d === "string") { await S.set("out_dir", d); S.boot!.output_dir = d; }
  }
  async function exp() {
    const p = await saveDialog({ defaultPath: "ai-video-studio-settings.json" });
    if (p) await S.run(() => api.exportSettings(p));
  }
  async function imp() {
    const p = await pickDialog({ filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof p === "string" && (await S.run(() => api.importSettings(p))) !== undefined) {
      await S.init(); S.notify(S.t("saved"));
    }
  }
  const setTheme = (t: Theme) => { S.applyTheme(t); S.set("theme", t); };
  const setLang = (l: Lang) => { S.lang = l; S.set("lang", l); };
</script>

{#snippet keyRow(name: string, label: string, val: string, set: (v: string) => void, help: string)}
  <label class="small dim" for={"k_" + name}>{label} <span class="pill {S.secrets[name] ? 'ok' : 'warn'}">{S.secrets[name] ? S.t("set") : S.t("not_set")}</span></label>
  <div class="row">
    <input id={"k_" + name} type="password" value={val} oninput={(e) => set(e.currentTarget.value)} placeholder={S.secrets[name] ? "••••••••••••" : ""} autocomplete="off" />
    <button onclick={async () => { await saveKey(name, val); set(""); }} disabled={!val}>{S.t("save")}</button>
  </div>
  {#if help}<div class="small dim">{help}</div>{/if}
{/snippet}

<div class="page form">
  <h1>{S.t("nav_settings")}</h1>
  <div class="card">
    <h2>{S.t("default_provider")}</h2>
    <div class="row">
      <select value={S.settings.provider ?? "vast"} onchange={(e) => S.set("provider", e.currentTarget.value)} style="width:auto">
        <option value="local">{S.t("run_pc")}</option>
        <option value="vast">{S.t("run_cloud")}</option>
      </select>
      <button onclick={() => (S.wizard = true)}>{S.t("rerun_wizard")}</button>
    </div>
  </div>

  <div class="card">
    <h2>{S.t("language")}</h2>
    <div class="row" style="flex-wrap:wrap">
      {#each LANGS as l}
        <button class:primary={S.lang === l.code} onclick={() => setLang(l.code)}>{l.name}</button>
      {/each}
    </div>
    <h2 style="margin-top:14px">{S.t("theme")}</h2>
    <div class="row">
      <button class:primary={S.theme === "dark"} onclick={() => setTheme("dark")}>{S.t("theme_dark")}</button>
      <button class:primary={S.theme === "light"} onclick={() => setTheme("light")}>{S.t("theme_light")}</button>
      <button class:primary={S.theme === "system"} onclick={() => setTheme("system")}>{S.t("theme_system")}</button>
    </div>
  </div>

  <div class="card">
    <h2>{ct(S.lang, "prov_title")}</h2>
    <p class="small dim">{ct(S.lang, "prov_note")}</p>
    {#each provKeys as k}
      <div class="field">
        <label for={k.name}>{k.label} <span class="pill {S.secrets[k.name] ? 'ok' : 'warn'}">{S.secrets[k.name] ? S.t("set") : S.t("not_set")}</span></label>
        <div class="row">
          <input id={k.name} type="password" bind:value={provVals[k.name]} placeholder={S.secrets[k.name] ? "••••••••••••" : ""} autocomplete="off" />
          <button onclick={() => saveProv(k)} disabled={!provVals[k.name]}>{k.hosted ? ct(S.lang, "save_test") : S.t("save")}</button>
        </div>
        <div class="small dim">{k.help}</div>
        {#if provRes[k.name]}<div class="msg {provRes[k.name].ok ? 'ok' : 'err'}">{provRes[k.name].text}</div>{/if}
      </div>
    {/each}
  </div>

  <div class="card">
    <h2>{S.t("keys_title")}</h2>
    <p class="small dim">{S.t("keys_note")}</p>

    <div class="field">
      <label for="vk">{S.t("vast_key")} <span class="pill {S.secrets.vast_api_key ? 'ok' : 'warn'}">{S.secrets.vast_api_key ? S.t("set") : S.t("not_set")}</span></label>
      <div class="row">
        <input id="vk" type="password" bind:value={vast} placeholder={S.secrets.vast_api_key ? "••••••••••••" : ""} autocomplete="off" />
        <button onclick={async () => { await saveKey("vast_api_key", vast); vast = ""; }} disabled={!vast}>{S.t("save")}</button>
      </div>
      <div class="small dim">{S.t("vast_key_help")}</div>
    </div>
    <div class="field">
      <label for="ep">{S.t("endpoint_name")}</label>
      <div class="row">
        <input id="ep" bind:value={endpoint} onchange={() => S.set("endpoint_name", endpoint.trim())} />
        <button onclick={() => test("vast")} disabled={busy === "vast"}>{busy === "vast" ? S.t("testing") : S.t("test")}</button>
        <button onclick={zero}>{S.t("scale_zero")}</button>
      </div>
      {#if res.vast}<div class="msg {res.vast.ok ? 'ok' : 'warn'}">{res.vast.text}</div>{/if}
      {#if S.secrets.vast_api_key && res.vast && !res.vast.ok && res.vast.text.includes("endpoint")}<button onclick={setupVast} disabled={busy === "vast"}>{S.t("setup_btn")}</button>{/if}
    </div>
    <div class="msg">{S.t("r2_note")}</div>

  </div>

  <div class="card">
    <h2>{S.t("provider_title")}</h2>
    <p class="small dim">{S.t("token_note")}</p>

    <div class="field">
      <label class="row"><input type="radio" name="prov" checked={provider === "cc"} onchange={() => S.set("enhance_provider", "cc")} /> {S.t("provider_cc")}</label>
      {#if provider === "cc"}
        <div class="sub">
          {#if S.boot?.claude_cli}
            <div class="small"><span class="pill ok">{S.t("cc_found")}</span> <span class="dim">{S.boot.claude_cli}</span></div>
          {:else}
            <div class="msg warn">{S.t("cc_missing")}</div>
          {/if}
          <div class="row" style="margin-top:8px">
            <select value={S.settings.claude_model ?? "haiku"} onchange={(e) => S.set("claude_model", e.currentTarget.value)}>
              {#each S.boot?.cli_models ?? [] as m}<option value={m}>{m}</option>{/each}
            </select>
          </div>
        </div>
      {/if}
    </div>

    <div class="field">
      <label class="row"><input type="radio" name="prov" checked={provider === "gemini"} onchange={() => S.set("enhance_provider", "gemini")} /> {S.t("provider_gemini")}</label>
      {#if provider === "gemini"}
        <div class="sub">
          {@render keyRow("gemini_api_key", S.t("gemini_key"), geminiKey, (v) => (geminiKey = v), S.t("gemini_help"))}
          <label class="small dim" for="gm">{S.t("model")}</label>
          <ModelSelect id="gm" provider="gemini" value={S.settings.gemini_model ?? "gemini-2.5-flash"} onchange={(v) => S.set("gemini_model", v)} />
        </div>
      {/if}
    </div>

    <div class="field">
      <label class="row"><input type="radio" name="prov" checked={provider === "openai"} onchange={() => S.set("enhance_provider", "openai")} /> {S.t("provider_openai")}</label>
      {#if provider === "openai"}
        <div class="sub">
          <label class="small dim" for="ob">{S.t("base_url")}</label>
          <input id="ob" value={S.settings.openai_base_url ?? "https://api.openai.com/v1"} onchange={(e) => S.set("openai_base_url", e.currentTarget.value.trim())} />
          <label class="small dim" for="om" style="margin-top:8px;display:block">{S.t("model")}</label>
          <ModelSelect id="om" provider="openai" value={S.settings.openai_model ?? "gpt-4o-mini"} onchange={(v) => S.set("openai_model", v)} />
          <div style="margin-top:8px">{@render keyRow("openai_api_key", S.t("openai_key"), openaiKey, (v) => (openaiKey = v), "")}</div>
        </div>
      {/if}
    </div>

    <div class="field">
      <label class="row"><input type="radio" name="prov" checked={provider === "api"} onchange={() => S.set("enhance_provider", "api")} /> {S.t("provider_api")}</label>
      {#if provider === "api"}
        <div class="sub">
          {@render keyRow("anthropic_api_key", S.t("anth_key"), anth, (v) => (anth = v), S.t("anth_key_help"))}
          <select value={S.settings.anthropic_model ?? S.boot?.anthropic_models[0]} onchange={(e) => S.set("anthropic_model", e.currentTarget.value)}>
            {#each S.boot?.anthropic_models ?? [] as m}<option value={m}>{m}</option>{/each}
          </select>
        </div>
      {/if}
    </div>

    <div class="row">
      <button onclick={testLlm} disabled={busy === "llm"}>{busy === "llm" ? S.t("testing") : S.t("test")}</button>
    </div>
    {#if res.llm}<div class="msg {res.llm.ok ? 'ok' : 'err'}">{res.llm.text}</div>{/if}
  </div>

  <div class="card">
    <h2>{S.t("delivery_title")}</h2>
    <div class="field">
      <label class="row"><input type="radio" name="del" checked={S.settings.delivery !== "cloud"} onchange={() => S.set("delivery", "local")} /> {S.t("delivery_local")}</label>
      <label class="row"><input type="radio" name="del" checked={S.settings.delivery === "cloud"} onchange={() => S.set("delivery", "cloud")} /> {S.t("delivery_cloud")}</label>
    </div>
    <p class="small dim">{S.t("delivery_note")}</p>
  </div>

  <div class="card">
    <h2>{S.t("out_dir")}</h2>
    <div class="row">
      <input value={S.boot?.output_dir} readonly />
      <button onclick={chooseDir}>{S.t("choose")}</button>
      <button onclick={() => S.run(() => api.reveal(S.boot!.output_dir))}>{S.t("open_folder")}</button>
    </div>
  </div>

  <div class="card">
    <h2>{S.t("share_title")}</h2>
    <p class="small dim">{S.t("share_note")}</p>
    <div class="row"><button onclick={exp}>{S.t("export")}</button><button onclick={imp}>{S.t("import")}</button></div>
  </div>

  <div class="card">
    <h2>{S.lang === "el" ? "Σύνδεση με Claude Desktop (προαιρετική)" : "Claude Desktop connection (optional)"}</h2>
    <p class="small dim">{S.lang === "el"
      ? "Απενεργοποιημένη από προεπιλογή: τίποτα δεν ακούει στον υπολογιστή σου. Αν την ανοίξεις, η εφαρμογή ακούει μόνο στο ίδιο PC με μυστικό κωδικό ώστε η επέκταση MCP του Claude Desktop να δίνει εντολές. Ισχύει αμέσως."
      : "Off by default: nothing listens on your computer. If you turn it on, the app listens on this PC only, behind a secret token, so the Claude Desktop MCP extension can send it commands. Takes effect immediately."}</p>
    <label class="row"><input type="checkbox" checked={S.settings.api_enabled === "1"} onchange={(e) => S.set("api_enabled", e.currentTarget.checked ? "1" : "0")} />
      {S.lang === "el" ? "Να επιτρέπεται η σύνδεση με Claude Desktop" : "Allow the Claude Desktop connection"}</label>
  </div>

  <div class="card">
    <h2>{ct(S.lang, "about_title")}</h2>
    <p>{ct(S.lang, "about_text")} <a href="https://theslopvibe.com" target="_blank" rel="noreferrer">theslopvibe.com</a>. v{S.boot ? "0.6.1" : ""}</p>
    <p class="small dim">{S.lang === "el"
      ? "Ο ενσωματωμένος editor video είναι το OpenCut (ανοιχτός κώδικας, άδεια MIT, από τους συνεργάτες του OpenCut). Δεν είναι δικό μας έργο· το πακετάρουμε χωρίς αλλαγές στη λειτουργία του. Ο editor εικόνας είναι το miniPaint (MIT, ViliusL), με δικό μας μενού AI."
      : "The built-in video editor is OpenCut (open source, MIT license, by the OpenCut contributors). It is not our work; we bundle it with no change to how it works. The image editor is miniPaint (MIT, by ViliusL) with our own AI menu."}
      <a href="https://github.com/opencut-app/opencut-classic" target="_blank" rel="noreferrer">github.com/opencut-app/opencut-classic</a></p>
    <div class="row">
      <button onclick={updateApp} disabled={updBusy}>{S.lang === "el" ? "Έλεγχος για ενημέρωση" : "Check for updates"}</button>
      {#if upd}<span class="small dim">{upd}</span>{/if}
    </div>
    <div class="row">
      <a class="btn" href="https://ko-fi.com/slopvibe" target="_blank" rel="noreferrer">☕ {S.lang === "el" ? "Κέρνα μας έναν καφέ" : "Buy us a coffee"}</a>
      <a class="btn ghost" href="https://theslopvibe.com/free-apps" target="_blank" rel="noreferrer">{S.lang === "el" ? "Περισσότερες δωρεάν εφαρμογές" : "More free apps"}</a>
      <a class="btn ghost" href="https://theslopvibe.com/contact" target="_blank" rel="noreferrer">{S.lang === "el" ? "Επικοινωνία" : "Hire us / contact"}</a>
    </div>
    <p class="small dim">{ct(S.lang, "about_models")}</p>
  </div>
</div>

<style>
  .sub { margin: 6px 0 4px 24px; }
</style>
