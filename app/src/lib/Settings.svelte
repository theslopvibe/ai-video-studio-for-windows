<script lang="ts">
  import { open as pickDialog, save as saveDialog, ask as askDialog } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import { S } from "./store.svelte";
  import type { Lang } from "./i18n";
  import { LANGS } from "./translations";
  import type { Theme } from "./store.svelte";
  import { ct } from "./chatStrings";
  import ModelSelect from "./ModelSelect.svelte";
  import RunpodPanel from "./RunpodPanel.svelte";
  import { check as checkUpdate } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  let conn = $state<{ extension: string; installer_bat: string; exe: string; claude_code_cmd: string; folder: string } | null>(null);
  let ccRes = $state<string[]>([]);
  async function connectClients() {
    busy = "cc";
    try { ccRes = await api.connectClients(); } catch (e) { ccRes = [String(e)]; }
    busy = "";
  }
  api.connectInfo().then((c) => (conn = c)).catch(() => {});
  async function copyText(t: string) { await navigator.clipboard.writeText(t).catch(() => {}); S.notify(S.t("copied")); }

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

  let rpKey = $state("");
  let rpRes = $state<{ ok: boolean; text: string } | null>(null);
  let anth = $state("");
  let geminiKey = $state("");
  let openaiKey = $state("");
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
  // RunPod: "Save and test" saves the key, then asks RunPod's /health for the endpoint (key + endpoint id are checked together)
  async function testRunpod() {
    busy = "rp"; rpRes = null;
    try { rpRes = { ok: true, text: await api.hostedTest("runpod") }; } catch (e) { rpRes = { ok: false, text: String(e) }; }
    busy = "";
    S.refreshRunpod();
  }
  async function saveRunpodKey() {
    await saveKey("runpod_api_key", rpKey); rpKey = "";
    if (S.secrets.runpod_api_key) await testRunpod();
  }
  async function saveEndpointId(v: string) {
    await S.set("runpod_endpoint_id", v.trim());
    S.refreshRunpod();
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
      <select value={S.provider} onchange={(e) => S.set("provider", e.currentTarget.value)} style="width:auto">
        <option value="runpod">{S.t("run_cloud")}</option>
        <option value="local">{S.t("run_pc")}</option>
      </select>
      <button onclick={() => (S.wizard = true)}>{S.t("rerun_wizard")}</button>
    </div>
  </div>

  <div class="card">
    <h2>{ct(S.lang, "rp_title")}</h2>
    <p class="small dim">{ct(S.lang, "rp_intro")}</p>
    <div class="field">
      <label for="rpk">{ct(S.lang, "rp_key_label")} <span class="pill {S.secrets.runpod_api_key ? 'ok' : 'warn'}">{S.secrets.runpod_api_key ? S.t("set") : S.t("not_set")}</span></label>
      <div class="row">
        <input id="rpk" type="password" bind:value={rpKey} placeholder={S.secrets.runpod_api_key ? "••••••••••••" : ""} autocomplete="off" />
        <button onclick={saveRunpodKey} disabled={!rpKey || busy === "rp"}>{ct(S.lang, "save_test")}</button>
      </div>
      <div class="small dim">{ct(S.lang, "rp_key_help")}</div>
    </div>
    <div class="field">
      <label for="rpe">{ct(S.lang, "rp_endpoint")}</label>
      <div class="row">
        <input id="rpe" value={S.settings.runpod_endpoint_id ?? ""} onchange={(e) => saveEndpointId(e.currentTarget.value)} autocomplete="off" />
        <button onclick={testRunpod} disabled={busy === "rp" || !S.runpodReady}>{busy === "rp" ? S.t("testing") : ct(S.lang, "rp_test")}</button>
      </div>
      <div class="small dim">{ct(S.lang, "rp_endpoint_help")}</div>
    </div>
    <div class="field">
      <label for="rpd">{ct(S.lang, "rp_rate")}</label>
      <input id="rpd" type="number" min="0" step="0.01" placeholder="1.22" value={S.settings.runpod_gpu_dph ?? ""} onchange={(e) => S.set("runpod_gpu_dph", e.currentTarget.value.trim())} style="width:120px" />
      <div class="small dim">{ct(S.lang, "rp_rate_help")}</div>
    </div>
    <div class="field">
      <label for="rpc">{S.lang === "el" ? "Όριο δαπάνης RunPod ανά συνεδρία (USD)" : "RunPod spend limit per session (USD)"}</label>
      <input id="rpc" type="number" min="0" step="1" placeholder="5" value={S.settings.runpod_cap_usd ?? ""} onchange={(e) => S.set("runpod_cap_usd", e.currentTarget.value.trim())} style="width:120px" />
      <div class="small dim">{S.lang === "el" ? "Όταν ξοδευτεί αυτό το ποσό από το άνοιγμα της εφαρμογής, δεν ξεκινά άλλη δουλειά. Προεπιλογή 5, 0 = χωρίς όριο. Αλλάζει μόνο εδώ, ποτέ από το Claude ή το ZCode." : "When this much has been spent since the app was opened, no more jobs start. Default 5, 0 = no limit. It can only be changed here, never from Claude or ZCode."}</div>
    </div>
    {#if rpRes}<div class="msg {rpRes.ok ? 'ok' : 'err'}">{rpRes.text}</div>{/if}
    <RunpodPanel />
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
    <h2>{S.lang === "el" ? "Σύνδεση με Claude (Claude Desktop ή Claude Code)" : "Connect to Claude (Claude Desktop or Claude Code)"}</h2>
    <p class="small dim">{S.lang === "el"
      ? "Τι κάνει: μπορείς να γράφεις στο Claude με απλά λόγια, π.χ. «φτιάξε ένα βίντεο 5 δευτερολέπτων με Wan 2.2 από αυτή τη φωτογραφία», και το Claude χρησιμοποιεί αυτό το πρόγραμμα για να το φτιάξει. Μπορεί επίσης να επιλέγει μοντέλο, να λέει πόσο θα κοστίσει, και να ξυπνά ή να βάζει για ύπνο το RunPod."
      : "What it does: you can talk to Claude in plain words, e.g. “make a 5-second video with Wan 2.2 from this photo”, and Claude uses this program to make it. It can also pick the model, tell you the price, and wake up or put RunPod to sleep."}</p>
    <p class="small dim">{S.lang === "el"
      ? "Πώς δουλεύει: Claude → σύνδεση (extension ή εντολή) → αυτό το πρόγραμμα, μόνο μέσα στον υπολογιστή σου και με μυστικό κωδικό → το πρόγραμμα στέλνει τη δουλειά στο RunPod endpoint σου (ή τρέχει στο PC σου) και σώζει το βίντεο στον φάκελό σου. Το Claude ρωτά πριν ξοδέψει χρήματα. Το RunPod ξυπνά μόνο του για κάθε δουλειά (το πρώτο βίντεο αργεί 1–3 λεπτά παραπάνω) και κοιμάται όταν η ουρά μείνει άδεια για λίγα λεπτά. Το πρόγραμμα πρέπει να είναι ανοιχτό (μπορεί και ελαχιστοποιημένο)."
      : "How it works: Claude → connection (extension or command) → this program, only inside your computer and behind a secret token → the program sends the job to your RunPod endpoint (or runs on your PC) and saves the video in your folder. Claude asks before spending money. RunPod wakes up by itself for each job (the first video takes 1–3 minutes longer) and goes back to sleep after the queue has been empty for a few minutes. The program must be open (minimised is fine)."}</p>
    <p><b>{S.lang === "el" ? "Βήμα 1" : "Step 1"}</b> — <label class="row" style="display:inline-flex"><input type="checkbox" checked={S.settings.api_enabled === "1"} onchange={(e) => S.set("api_enabled", e.currentTarget.checked ? "1" : "0")} />
      {S.lang === "el" ? "Να επιτρέπεται η σύνδεση με το Claude (ακούει μόνο σε αυτό το PC)" : "Allow the Claude connection (listens on this PC only)"}</label></p>
    <p><b>{S.lang === "el" ? "Βήμα 2 — Σύνδεση με ένα κουμπί (Claude Desktop και ZCode/GLM)" : "Step 2 — Connect with one button (Claude Desktop and ZCode/GLM)"}</b></p>
    <div class="row">
      <button class="primary" disabled={busy === "cc"} onclick={connectClients}>{S.lang === "el" ? "Σύνδεση Claude και ZCode" : "Connect Claude and ZCode"}</button>
    </div>
    {#each ccRes as line}<div class="msg ok small">{line}</div>{/each}
    <p class="small dim">{S.lang === "el" ? "Κλείσε εντελώς και ξανάνοιξε το Claude Desktop / ZCode μετά. Το κλειδί RunPod δεν περνάει ποτέ σε αυτά: μένει μόνο σε αυτή την εφαρμογή. Αν δεν δουλέψει, δες τις χειροκίνητες επιλογές παρακάτω." : "Close and reopen Claude Desktop / ZCode afterwards. Your RunPod key never goes to them: it stays inside this app only. If it does not work, use the manual options below."}</p>
    <p><b>{S.lang === "el" ? "Βήμα 2α — Claude Desktop" : "Step 2a — Claude Desktop"}</b></p>
    <p class="small dim">{S.lang === "el"
      ? "Επιλογή 1 (πιο εύκολη): πάτα «Δείξε το αρχείο extension» και σύρε το αρχείο .mcpb μέσα στο παράθυρο του Claude Desktop (ή διπλό κλικ), μετά «Install». Επιλογή 2: «Δείξε το Connect-Claude.bat» και διπλό κλικ· προσθέτει τη σύνδεση μόνο του, χωρίς αρχείο extension. Μετά κλείσε και ξανάνοιξε το Claude Desktop."
      : "Option 1 (easiest): press “Show extension file” and drag the .mcpb file into the Claude Desktop window (or double-click it), then “Install”. Option 2: “Show Connect-Claude.bat” and double-click it; it adds the connection by itself, no extension file needed. Then close and reopen Claude Desktop."}</p>
    <div class="row">
      <button disabled={!conn} onclick={() => conn && S.run(() => api.reveal(conn!.extension))}>{S.lang === "el" ? "Δείξε το αρχείο extension (.mcpb)" : "Show extension file (.mcpb)"}</button>
      <button disabled={!conn} onclick={() => conn && S.run(() => api.reveal(conn!.installer_bat))}>{S.lang === "el" ? "Δείξε το Connect-Claude.bat" : "Show Connect-Claude.bat"}</button>
    </div>
    <p><b>{S.lang === "el" ? "Βήμα 2γ — ZCode (GLM)" : "Step 2c — ZCode (GLM)"}</b></p>
    <p class="small dim">{S.lang === "el" ? "Για να δουλεύει το GLM όπως το Claude: πάτα το κουμπί και κάνε διπλό κλικ στο αρχείο. Προσθέτει τη σύνδεση στο ~/.zcode/cli/config.json. Μετά κλείσε και ξανάνοιξε το ZCode." : "To make GLM work like Claude: press the button and double-click the file. It adds the connection to ~/.zcode/cli/config.json. Then close and reopen ZCode."}</p>
    <div class="row"><button disabled={!conn} onclick={() => conn && S.run(() => api.reveal(conn!.folder + "\Connect-ZCode.bat"))}>{S.lang === "el" ? "Δείξε το Connect-ZCode.bat" : "Show Connect-ZCode.bat"}</button></div>
    <p><b>{S.lang === "el" ? "Βήμα 2β — Claude Code (τερματικό)" : "Step 2b — Claude Code (terminal)"}</b></p>
    <p class="small dim">{S.lang === "el"
      ? "Αντίγραψε την εντολή, άνοιξε ένα τερματικό και επικόλλησέ την μία φορά. Το Claude Code θα βλέπει τα εργαλεία του προγράμματος."
      : "Copy the command, open a terminal and paste it once. Claude Code will then see this program's tools."}</p>
    <div class="row">
      <input readonly style="flex:1" value={conn?.claude_code_cmd ?? ""} />
      <button disabled={!conn} onclick={() => conn && copyText(conn.claude_code_cmd)}>{S.lang === "el" ? "Αντιγραφή" : "Copy"}</button>
    </div>
    <p class="small dim"><b>{S.lang === "el" ? "Βήμα 3" : "Step 3"}</b> — {S.lang === "el"
      ? "Ρώτα το Claude, π.χ. «τι μοντέλα έχεις;» ή «φτιάξε βίντεο με Wan 2.2 από αυτή την εικόνα». Για να είσαι σίγουρος ότι δεν χρεώνεσαι πες «τελείωσα, βάλε το RunPod για ύπνο»."
      : "Ask Claude, e.g. “what models do you have?” or “make a video with Wan 2.2 from this image”. To be sure nothing is billed say “I’m done, put RunPod to sleep”."}</p>
  </div>

  <div class="card">
    <h2>{ct(S.lang, "about_title")}</h2>
    <p>{ct(S.lang, "about_text")} <a href="https://theslopvibe.com" target="_blank" rel="noreferrer">theslopvibe.com</a>. v{S.boot ? "0.9.2" : ""}</p>
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
