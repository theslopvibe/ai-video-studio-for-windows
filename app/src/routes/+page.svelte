<script lang="ts">
  import { onMount } from "svelte";
  import { S, type Page } from "$lib/store.svelte";
  import Generate from "$lib/Generate.svelte";
  import Queue from "$lib/Queue.svelte";
  import Library from "$lib/Library.svelte";
  import Projects from "$lib/Projects.svelte";
  import RunpodChip from "$lib/RunpodChip.svelte";
  import Gpu from "$lib/Gpu.svelte";
  import Models from "$lib/Models.svelte";
  import Settings from "$lib/Settings.svelte";
  import Guide from "$lib/Guide.svelte";
  import Editor from "$lib/Editor.svelte";
  import { isRtl } from "$lib/translations";
  import Logs from "$lib/Logs.svelte";
  import Wizard from "$lib/Wizard.svelte";
  import Chat from "$lib/Chat.svelte";

  let fatal = $state("");
  $effect(() => {
    document.documentElement.lang = S.lang;
    document.documentElement.dir = isRtl(S.lang) ? "rtl" : "ltr";
  });
  onMount(() => { S.init().catch((e) => (fatal = String(e))); });

  // Five places to go. Everything else lives as a tab inside Settings.
  type Rail = "create" | "assistant" | "editor" | "library" | "settings";
  const RAIL: { id: Rail; page: Page; key: "nav_generate" | "nav_chat" | "nav_editor" | "nav_library" | "nav_settings"; icon: string }[] = [
    { id: "create", page: "generate", key: "nav_generate", icon: '<path d="M12 3v4M12 17v4M3 12h4M17 12h4M6.3 6.3l2.4 2.4M15.3 15.3l2.4 2.4M17.7 6.3l-2.4 2.4M8.7 15.3l-2.4 2.4"/>' },
    { id: "assistant", page: "chat", key: "nav_chat", icon: '<path d="M4 5h16v11H9l-5 4z"/>' },
    { id: "editor", page: "editor", key: "nav_editor", icon: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M8 5v14M16 5v14M3 10h5M3 14h5M16 10h5M16 14h5"/>' },
    { id: "library", page: "library", key: "nav_library", icon: '<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><rect x="13" y="13" width="7" height="7" rx="1.5"/>' },
    { id: "settings", page: "settings", key: "nav_settings", icon: '<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>' },
  ];
  const SETTINGS_TABS: { page: Page; key: "nav_settings" | "nav_models" | "nav_gpu" | "nav_projects" | "nav_guide" | "nav_logs" }[] = [
    { page: "settings", key: "nav_settings" }, { page: "models", key: "nav_models" }, { page: "gpu", key: "nav_gpu" },
    { page: "projects", key: "nav_projects" }, { page: "guide", key: "nav_guide" }, { page: "logs", key: "nav_logs" },
  ];
  // the GPU & cost tab only exists for Vast (RunPod has no cards to choose)
  const TABS = $derived(SETTINGS_TABS.filter((t) => t.page !== "gpu" || S.cloud === "vast"));
  const group = (p: Page): Rail =>
    p === "generate" ? "create" : p === "chat" ? "assistant" : p === "editor" ? "editor" : p === "library" || p === "queue" ? "library" : "settings";
  const inSettings = $derived(group(S.page) === "settings");

  function pick(id: string) { S.projectId = id; S.set("last_project", id); }
</script>

{#if fatal}
  <div class="page"><div class="msg err">{fatal}</div></div>
{:else if S.ready && S.wizard}
  <Wizard />
  {#if S.toast}<div class="toast {S.toast.err ? 'err' : ''}">{S.toast.text}</div>{/if}
{:else if S.ready}
  <div class="shell">
    <nav class="rail" aria-label="Main">
      <div class="mark" class:live={S.pending.length > 0} title="AI Video Studio For Windows"><span></span></div>
      {#each RAIL as n}
        <button class="rbtn" class:on={group(S.page) === n.id} aria-current={group(S.page) === n.id ? "page" : undefined} onclick={() => (S.page = n.page)}>
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html n.icon}</svg>
          <span class="lbl">{S.t(n.key)}</span>
          {#if n.id === "library" && S.pending.length}<i class="badge">{S.pending.length}</i>{/if}
        </button>
      {/each}
    </nav>

    <div class="col">
      <header class="top">
        <label class="proj small dim">
          {S.t("project")}
          <select value={S.projectId} onchange={(e) => pick(e.currentTarget.value)}>
            {#each S.projects as p}<option value={p.id}>{p.name}</option>{/each}
          </select>
        </label>
        <button class="ghost small" title={S.t("manage")} onclick={() => (S.page = "projects")}>＋ {S.t("manage")}</button>
        <span class="grow"></span>
        <RunpodChip />
        {#if S.pending.length}
          <button class="tally" onclick={() => (S.page = "library")}><span class="dot"></span>{S.t("now_running")} · {S.pending.length}</button>
        {/if}
      </header>

      {#if inSettings}
        <div class="tabs" role="tablist">
          {#each TABS as t}
            <button role="tab" aria-selected={S.page === t.page} class:on={S.page === t.page} onclick={() => (S.page = t.page)}>{S.t(t.key)}</button>
          {/each}
        </div>
      {/if}

      <main>
        {#if S.page === "generate"}<Generate />
        {:else if S.page === "chat"}<Chat />
        {:else if S.page === "library" || S.page === "queue"}
          <div class="page">
            <Queue embedded />
            <Library embedded />
          </div>
        {:else if S.page === "projects"}<Projects />
        {:else if S.page === "gpu" && S.cloud === "vast"}<Gpu />
        {:else if S.page === "models"}<Models />
        {:else if S.page === "guide"}<Guide />
        {:else if S.page === "editor"}<Editor />
        {:else if S.page === "logs"}<Logs />
        {:else}<Settings />{/if}
      </main>
    </div>
  </div>
  {#if S.toast}<div class="toast {S.toast.err ? 'err' : ''}">{S.toast.text}</div>{/if}
{/if}

<style>
  .shell { display: grid; grid-template-columns: 88px 1fr; height: 100vh; }
  .rail { background: var(--panel); border-inline-end: 1px solid var(--line); padding: 14px 8px; display: flex; flex-direction: column; align-items: stretch; gap: 4px; }
  .mark { align-self: center; width: 30px; height: 30px; border-radius: 50%; border: 2px solid var(--line); display: grid; place-items: center; margin: 2px 0 14px; }
  .mark span { width: 12px; height: 12px; border-radius: 50%; background: var(--dim); opacity: .5; }
  .mark.live { border-color: var(--accent); }
  .mark.live span { background: var(--accent); opacity: 1; animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: .35; } }
  .rbtn { position: relative; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 10px 4px 8px; background: transparent; border: 0; border-radius: 10px; color: var(--dim); }
  .rbtn:hover { background: var(--panel2); color: var(--text); }
  .rbtn.on { background: var(--sel); color: var(--text); }
  .rbtn.on svg { color: var(--accent); }
  .lbl { font-size: 11px; line-height: 1.2; text-align: center; overflow-wrap: anywhere; }
  .badge { position: absolute; top: 4px; inset-inline-end: 12px; background: var(--accent2); color: var(--on-accent); border-radius: 99px; font-size: 10px; font-style: normal; font-weight: 700; padding: 0 6px; }
  .col { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .top { display: flex; align-items: center; gap: 10px; padding: 8px 24px; border-bottom: 1px solid var(--line); flex: none; }
  .proj { display: flex; align-items: center; gap: 8px; }
  .proj select { width: auto; max-width: 220px; padding: 5px 8px; }
  .tally { display: flex; align-items: center; gap: 8px; border-color: var(--accent); color: var(--text); padding: 4px 12px; border-radius: 99px; }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--accent); animation: pulse 1.6s ease-in-out infinite; }
  .tabs { display: flex; gap: 2px; padding: 0 24px; border-bottom: 1px solid var(--line); flex: none; overflow-x: auto; }
  .tabs button { background: transparent; border: 0; border-bottom: 2px solid transparent; border-radius: 0; padding: 11px 14px; color: var(--dim); white-space: nowrap; }
  .tabs button:hover { color: var(--text); }
  .tabs button.on { color: var(--text); border-bottom-color: var(--accent); }
  main { flex: 1; min-height: 0; min-width: 0; overflow: hidden; }
  .toast { position: fixed; bottom: 18px; inset-inline-end: 18px; background: var(--panel2); border: 1px solid var(--ok); padding: 10px 14px; border-radius: 10px; max-width: 460px; z-index: 20; white-space: pre-wrap; }
  .toast.err { border-color: var(--err); }
</style>
