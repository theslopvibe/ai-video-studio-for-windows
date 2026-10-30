<script lang="ts">
  import type { Job } from "./api";
  import { api } from "./api";
  import { S } from "./store.svelte";

  let { job }: { job: Job } = $props();

  const steps = ["queued", "routing", "generating", "downloading", "done"] as const;
  const labels = { queued: "ph_queued", routing: "ph_routing", generating: "ph_generating", downloading: "ph_downloading", done: "ph_done" } as const;
  let idx = $derived(steps.indexOf(job.status as (typeof steps)[number]));
  let running = $derived(["routing", "generating", "downloading"].includes(job.status));
  let failed = $derived(job.status === "failed" || job.status === "cancelled");
</script>

<div class="card job">
  <div class="row">
    <div class="grow prompt" title={job.prompt}>{job.prompt}</div>
    {#if job.status === "queued" || running}
      <button class="danger" onclick={() => S.run(() => api.cancel(job.id))}>{S.t("cancel")}</button>
    {:else if failed}
      <button onclick={() => S.run(() => api.retry(job.id))}>{S.t("retry")}</button>
      <button class="ghost danger" onclick={() => S.run(() => api.deleteJob(job.id, false).then(() => (S.jobs = S.jobs.filter((j) => j.id !== job.id))))}>{S.t("remove")}</button>
    {/if}
  </div>
  {#if !failed}
    <div class="steps">
      {#each steps as s, i}
        <div class="step" class:on={i === idx} class:past={i < idx}>
          <span class="dot" class:pulse={i === idx && running}></span>{S.t(labels[s])}
        </div>
      {/each}
    </div>
    <div class="small dim">{job.message}</div>
    {#if job.status === "routing"}<div class="small dim">{S.t("cold_hint")}</div>{/if}
  {:else}
    <div class="small"><span class="pill {job.status === 'failed' ? 'err' : ''}">{S.t(job.status === "failed" ? "st_failed" : "st_cancelled")}</span></div>
    {#if job.error}<div class="msg err">{job.error}</div>{/if}
  {/if}
</div>

<style>
  .job { padding: 12px 14px; margin-bottom: 10px; }
  .prompt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .steps { display: flex; gap: 6px; margin: 10px 0 6px; flex-wrap: wrap; }
  .step { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--dim); padding: 3px 10px 3px 8px; border-radius: 99px; border: 1px solid var(--line); }
  .step.past { color: var(--ok); } .step.on { color: var(--text); border-color: var(--accent); background: var(--sel); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: currentColor; opacity: .6; }
  .step.on .dot { background: var(--accent); opacity: 1; }
  .pulse { animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { transform: scale(1.7); opacity: .4; } }
</style>
