<script lang="ts">
  import { api } from "./api";
  import { S } from "./store.svelte";

  let text = $state("");
  let auto = $state(true);
  let report = $state("");
  let box: HTMLPreElement | undefined = $state();

  async function load() {
    const t = await api.logsTail(300).catch(() => "");
    const stick = !box || box.scrollTop + box.clientHeight >= box.scrollHeight - 30;
    text = t;
    if (stick) queueMicrotask(() => box && (box.scrollTop = box.scrollHeight));
  }
  $effect(() => {
    load();
    if (!auto) return;
    const t = setInterval(load, 3000);
    return () => clearInterval(t);
  });

  async function makeReport() {
    const p = await S.run(() => api.logsReport());
    if (!p) return;
    report = p;
    await navigator.clipboard.writeText(p).catch(() => {});
    await S.run(() => api.reveal(p));
    load();
  }
  async function copy() { await navigator.clipboard.writeText(text); S.notify(S.t("copied")); }
  async function folder() { const d = await api.logsDir(); if (d) S.run(() => api.reveal(d)); }
</script>

<div class="page">
  <h1>{S.t("nav_logs")}</h1>
  <p class="dim">{S.t("logs_intro")}</p>

  <div class="card">
    <div class="row wrap">
      <button class="primary" onclick={makeReport}>{S.t("logs_report")}</button>
      <button onclick={copy}>{S.t("logs_copy")}</button>
      <button onclick={folder}>{S.t("logs_folder")}</button>
      <button class="ghost" onclick={load}>{S.t("logs_refresh")}</button>
      <label class="row small dim" style="margin-left:auto"><input type="checkbox" bind:checked={auto} />{S.t("logs_auto")}</label>
    </div>
    {#if report}<div class="msg ok">{S.t("logs_report_done")}<br /><b>{report}</b></div>{/if}
    <pre bind:this={box} style="max-height:60vh;overflow:auto;margin-top:10px;font-size:12px;white-space:pre-wrap">{text || S.t("logs_empty")}</pre>
  </div>
</div>
