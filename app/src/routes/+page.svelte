<script lang="ts">
  import { onMount } from "svelte";
  import { S, type Page } from "$lib/store.svelte";
  import Generate from "$lib/Generate.svelte";
  import Queue from "$lib/Queue.svelte";
  import Library from "$lib/Library.svelte";
  import Projects from "$lib/Projects.svelte";
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

  const nav: { id: Page; key: "nav_generate" | "nav_chat" | "nav_queue" | "nav_library" | "nav_projects" | "nav_gpu" | "nav_models" | "nav_guide" | "nav_editor" | "nav_logs" | "nav_settings"; icon: string }[] = [
    { id: "generate", key: "nav_generate", icon: "✦" },
    { id: "chat", key: "nav_chat", icon: "✎" },
    { id: "editor", key: "nav_editor", icon: "✂" },
    { id: "library", key: "nav_library", icon: "▦" },
    { id: "queue", key: "nav_queue", icon: "☰" },
    { id: "projects", key: "nav_projects", icon: "▤" },
    { id: "models", key: "nav_models", icon: "◈" },
    { id: "gpu", key: "nav_gpu", icon: "⚡" },
    { id: "guide", key: "nav_guide", icon: "?" },
    { id: "logs", key: "nav_logs", icon: "≡" },
    { id: "settings", key: "nav_settings", icon: "⚙" },
  ];

  function pick(id: string) { S.projectId = id; S.set("last_project", id); }
</script>

{#if fatal}
  <div class="page"><div class="msg err">{fatal}</div></div>
{:else if S.ready && S.wizard}
  <Wizard />
  {#if S.toast}<div class="toast {S.toast.err ? 'err' : ''}">{S.toast.text}</div>{/if}
{:else if S.ready}
  <div class="shell">
    <aside>
      <div class="brand">AI Video Studio For Windows</div>
      <div class="proj">
        <div class="small dim">{S.t("project")}</div>
        <select value={S.projectId} onchange={(e) => pick(e.currentTarget.value)}>
          {#each S.projects as p}<option value={p.id}>{p.name}</option>{/each}
        </select>
        <button class="ghost small" style="margin-top:6px" onclick={() => (S.page = "projects")}>＋ {S.t("manage")}</button>
      </div>
      <nav>
        {#each nav as n}
          {#if n.id === "projects"}<div class="navsep small dim">{S.t("advanced")}</div>{/if}
          <button class="navbtn" class:on={S.page === n.id} onclick={() => (S.page = n.id)}>
            <span class="ic">{n.icon}</span>{S.t(n.key)}
            {#if n.id === "queue" && S.pending.length}<span class="badge">{S.pending.length}</span>{/if}
          </button>
        {/each}
      </nav>
    </aside>
    <main>
      {#if S.page === "generate"}<Generate />
      {:else if S.page === "chat"}<Chat />
      {:else if S.page === "queue"}<Queue />
      {:else if S.page === "library"}<Library />
      {:else if S.page === "projects"}<Projects />
      {:else if S.page === "gpu"}<Gpu />
      {:else if S.page === "models"}<Models />
      {:else if S.page === "guide"}<Guide />
      {:else if S.page === "editor"}<Editor />
      {:else if S.page === "logs"}<Logs />
      {:else}<Settings />{/if}
    </main>
  </div>
  {#if S.toast}<div class="toast {S.toast.err ? 'err' : ''}">{S.toast.text}</div>{/if}
{/if}

<style>
  .shell { display: grid; grid-template-columns: 220px 1fr; height: 100vh; }
  aside { background: var(--panel); border-right: 1px solid var(--line); padding: 16px 12px; display: flex; flex-direction: column; gap: 16px; }
  .brand { font-weight: 700; letter-spacing: .2px; padding: 0 6px; }
  .proj { padding: 0 4px; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  .navsep { margin: 10px 10px 2px; text-transform: uppercase; letter-spacing: .06em; font-size: 10px; }
  .navbtn { text-align: left; background: transparent; border: 0; border-radius: 8px; padding: 9px 10px; display: flex; align-items: center; gap: 10px; color: var(--dim); }
  .navbtn:hover { background: var(--panel2); color: var(--text); } .navbtn.on { background: var(--sel); color: var(--text); }
  .ic { width: 18px; text-align: center; }
  .badge { margin-left: auto; background: var(--accent2); color: #fff; border-radius: 99px; font-size: 11px; padding: 0 7px; }
  main { min-width: 0; overflow: hidden; }
  .toast { position: fixed; bottom: 18px; right: 18px; background: var(--panel2); border: 1px solid var(--ok); padding: 10px 14px; border-radius: 10px; max-width: 460px; z-index: 20; white-space: pre-wrap; }
  .toast.err { border-color: var(--err); }
</style>
