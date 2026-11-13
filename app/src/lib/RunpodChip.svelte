<script lang="ts">
  // Header status chip: asleep / awake / working, with the money spent this session and the same wake / sleep action.
  import { S } from "./store.svelte";
  import { ct } from "./chatStrings";

  const st = $derived(S.rp?.state);
  const label = $derived(st === "asleep" ? ct(S.lang, "rp_state_asleep") : st === "working" ? ct(S.lang, "rp_state_working") : st === "awake" ? ct(S.lang, "rp_state_awake") : ct(S.lang, "rp_state_unknown"));
  const tip = $derived(S.rpError || (st === "asleep" ? ct(S.lang, "rp_asleep_help") : st === "working" ? ct(S.lang, "rp_working_help") : ct(S.lang, "rp_awake_help")));
</script>

{#if S.runpodReady}
  <div class="chip" class:live={st === "awake" || st === "working"} class:work={st === "working"} title={tip}>
    <span class="dot"></span>
    <span>{S.rpName(S.rpSlot)}: {label}</span>
    {#if S.rp}<span class="dim">· ${S.rp.session_usd.toFixed(2)}</span>{/if}
    {#if st === "asleep"}
      <button class="ghost small" onclick={() => S.runpodWake()} disabled={S.rpBusy}>{ct(S.lang, "rp_wake")}</button>
    {:else if st}
      <button class="ghost small" onclick={() => S.runpodSleepAsk()} disabled={S.rpBusy}>{ct(S.lang, "rp_sleep_btn")}</button>
    {/if}
  </div>
{/if}

<style>
  .chip { display: flex; align-items: center; gap: 8px; border: 1px solid var(--line); border-radius: 99px; padding: 2px 6px 2px 12px; font-size: 13px; }
  .chip.live { border-color: var(--ok); }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--dim); opacity: .6; }
  .live .dot { background: var(--ok); opacity: 1; }
  .work .dot { animation: pulse 1.6s ease-in-out infinite; }
  .chip button { padding: 2px 10px; }
  @keyframes pulse { 50% { opacity: .35; } }
</style>
