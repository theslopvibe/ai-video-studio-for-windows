<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import { S } from "./store.svelte";

  const el = $derived(S.lang === "el");
  let url = $state<string | null>(null);
  let tries = $state(0);

  // the local editor server starts with the app; wait a moment if it is not up yet
  onMount(async () => {
    while (!url && tries < 20) {
      url = await api.editorUrl().catch(() => null);
      if (!url) { tries++; await new Promise((r) => setTimeout(r, 300)); }
    }
  });
</script>

<div class="ed">
  <div class="bar">
    <span class="small dim">
      {el ? "Επεξεργασία video: το" : "Video editor:"}
      <a href="https://github.com/opencut-app/opencut-classic" target="_blank" rel="noreferrer">OpenCut</a>
      {el ? "(ανοιχτός κώδικας, άδεια MIT), τρέχει τοπικά στον υπολογιστή σου." : "(open source, MIT license), running locally on your computer."}
    </span>
    <span class="grow"></span>
    <span class="small dim">{el ? "Τα clips σου:" : "Your clips:"}</span>
    <button class="ghost small" onclick={() => S.run(() => api.reveal(S.boot?.output_dir ?? ""))}>{el ? "Άνοιγμα φακέλου" : "Open output folder"}</button>
  </div>
  {#if url}
    <iframe title="OpenCut" src={url + "/projects/"} allow="clipboard-read; clipboard-write; fullscreen"></iframe>
  {:else if tries >= 20}
    <p class="msg err" style="margin:16px">{el ? "Ο editor δεν βρέθηκε σε αυτή την έκδοση της εφαρμογής." : "The editor is not part of this build of the app."}</p>
  {:else}
    <p class="dim" style="margin:16px">…</p>
  {/if}
</div>

<style>
  .ed { display: flex; flex-direction: column; height: 100%; }
  .bar { display: flex; gap: 8px; align-items: center; padding: 6px 12px; border-bottom: 1px solid var(--line); background: var(--panel); }
  .grow { flex: 1; }
  iframe { flex: 1; border: 0; width: 100%; min-height: 0; background: #0e0e0e; }
</style>
