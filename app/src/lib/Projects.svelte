<script lang="ts">
  import { open as pickDialog } from "@tauri-apps/plugin-dialog";
  import { api, type Project } from "./api";
  import { S } from "./store.svelte";

  let newName = $state("");
  let newFolder = $state("");
  let drafts = $state<Record<string, { name: string; folder: string; suffix: string; sidecar: boolean }>>({});

  // Editable copies of each project; created outside the template (Svelte forbids state writes while rendering).
  $effect(() => {
    for (const p of S.projects) {
      if (!drafts[p.id]) drafts[p.id] = { name: p.name, folder: p.folder, suffix: p.suffix, sidecar: p.sidecar };
    }
  });
  const dirty = (p: Project) => { const x = drafts[p.id]; return !!x && (x.name !== p.name || x.folder !== p.folder || x.suffix !== p.suffix || x.sidecar !== p.sidecar); };

  async function choose(set: (v: string) => void) {
    const r = await pickDialog({ directory: true });
    if (typeof r === "string") set(r);
  }
  async function create() {
    const list = await S.run(() => api.projectCreate(newName, newFolder));
    if (!list) return;
    S.projects = list;
    const created = list[list.length - 1];
    S.projectId = created.id; S.set("last_project", created.id);
    newName = ""; newFolder = "";
    S.notify(S.t("saved"));
  }
  async function save(p: Project) {
    const x = drafts[p.id];
    const list = await S.run(() => api.projectUpdate(p.id, x.name, x.folder, x.suffix, x.sidecar));
    if (list) { S.projects = list; delete drafts[p.id]; S.notify(S.t("saved")); }
  }
  async function remove(p: Project) {
    if (!confirm(S.t("delete_project_q"))) return;
    const list = await S.run(() => api.projectDelete(p.id));
    if (!list) return;
    S.projects = list;
    S.jobs = S.jobs.filter((j) => j.project_id !== p.id);
    if (S.projectId === p.id) { S.projectId = list[0].id; S.set("last_project", list[0].id); }
  }
</script>

<div class="page form">
  <h1>{S.t("projects_title")}</h1>
  <p class="dim">{S.t("projects_intro")}</p>

  <div class="card">
    <h2>{S.t("new_project_title")}</h2>
    <div class="field">
      <label for="np">{S.t("project_name")}</label>
      <input id="np" bind:value={newName} />
    </div>
    <div class="field">
      <label for="nf">{S.t("project_folder")}</label>
      <div class="row">
        <input id="nf" value={newFolder} placeholder={S.t("folder_default")} readonly />
        <button onclick={() => choose((v) => (newFolder = v))}>{S.t("choose")}</button>
        {#if newFolder}<button class="ghost" onclick={() => (newFolder = "")}>{S.t("reset_folder")}</button>{/if}
      </div>
    </div>
    <button class="primary" onclick={create} disabled={!newName.trim()}>{S.t("create")}</button>
  </div>

  {#each S.projects as p (p.id)}
    {@const x = drafts[p.id]}
    {#if x}
    <div class="card" class:current={p.id === S.projectId}>
      <div class="row">
        <input class="grow" aria-label={S.t("project_name")} bind:value={x.name} />
        <button class="ghost" onclick={() => { S.projectId = p.id; S.set("last_project", p.id); }} disabled={p.id === S.projectId}>
          {p.id === S.projectId ? "✓" : "↪"}
        </button>
        <button class="ghost danger" onclick={() => remove(p)} title={S.t("delete")}>🗑</button>
      </div>
      <div class="field" style="margin-top:10px">
        <label for={"pf" + p.id}>{S.t("project_folder")}</label>
        <div class="row">
          <input id={"pf" + p.id} value={x.folder} placeholder={p.dir || S.t("folder_default")} readonly />
          <button onclick={() => choose((v) => (x.folder = v))}>{S.t("choose")}</button>
          {#if x.folder}<button class="ghost" onclick={() => (x.folder = "")}>{S.t("reset_folder")}</button>{/if}
          <button onclick={() => S.run(() => api.reveal(p.dir))}>{S.t("open_folder")}</button>
        </div>
        <div class="small dim" style="margin-top:4px">{S.t("saving_to")}: {x.folder || p.dir}</div>
      </div>
      <div class="field">
        <label for={"ps" + p.id}>{S.t("style_suffix")}</label>
        <input id={"ps" + p.id} bind:value={x.suffix} placeholder={S.t("style_suffix_ph")} />
      </div>
      <label class="row small"><input type="checkbox" bind:checked={x.sidecar} /> {S.t("sidecar")}</label>
      <div class="row" style="margin-top:10px">
        <button class="primary" onclick={() => save(p)} disabled={!dirty(p) || !x.name.trim()}>{S.t("update")}</button>
      </div>
    </div>
    {/if}
  {/each}
</div>

<style>
  .current { border-color: var(--accent); }
</style>
