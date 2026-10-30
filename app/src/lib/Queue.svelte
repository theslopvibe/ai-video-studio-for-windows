<script lang="ts">
  import { api } from "./api";
  import { S } from "./store.svelte";
  import JobCard from "./JobCard.svelte";

  let list = $derived(S.jobs.filter((j) => j.status !== "done").slice().sort((a, b) => a.created_at - b.created_at));
  let finishedBad = $derived(list.filter((j) => j.status === "failed" || j.status === "cancelled"));

  async function clear() {
    for (const j of finishedBad) await api.deleteJob(j.id, false);
    S.jobs = S.jobs.filter((j) => !finishedBad.includes(j));
  }
</script>

<div class="page">
  <div class="row">
    <h1 class="grow">{S.t("nav_queue")}</h1>
    {#if finishedBad.length}<button onclick={() => S.run(clear)}>{S.t("clear_finished")}</button>{/if}
  </div>
  {#each list as j (j.id)}<JobCard job={j} />{:else}<p class="dim">{S.t("queue_empty")}</p>{/each}
</div>
