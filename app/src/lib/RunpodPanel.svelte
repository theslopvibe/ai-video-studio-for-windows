<script lang="ts">
  // The RunPod buttons: wake up / put to sleep, the state, the spend of this session and the auto-sleep options.
  // (They replace the old "start card" / "delete card" buttons: there are no rented cards any more.)
  import { onMount } from "svelte";
  import { S } from "./store.svelte";
  import { ct } from "./chatStrings";

  onMount(() => { S.refreshRunpod(); });
  const st = $derived(S.rp?.state);
  const help = $derived(st === "asleep" ? "rp_asleep_help" : st === "working" ? "rp_working_help" : "rp_awake_help");
  const sleepMin = $derived(S.settings.runpod_sleep_min ?? "5");
</script>

{#if !S.runpodReady}
  <p class="small dim">{ct(S.lang, "rp_need")}</p>
{:else}
  <p>
    <b class:ok-text={st && st !== "asleep"}>{st === "asleep" ? "🌙 " + ct(S.lang, "rp_state_asleep") : st === "working" ? "⚙ " + ct(S.lang, "rp_state_working") : st === "awake" ? "✔ " + ct(S.lang, "rp_state_awake") : "… " + ct(S.lang, "rp_state_unknown")}</b>
    {#if S.rp}<span class="small dim"> · {ct(S.lang, "rp_session")}: ${S.rp.session_usd.toFixed(3)}</span>{/if}
  </p>
  {#if st}<p class="small dim">{ct(S.lang, help)}</p>{/if}
  {#if S.rpError}<div class="msg err">{S.rpError}</div>{/if}
  <div class="row" style="flex-wrap:wrap">
    <button class="primary" onclick={() => S.runpodWake()} disabled={S.rpBusy || st !== "asleep"}>{ct(S.lang, "rp_wake")}</button>
    <button onclick={() => S.runpodSleepAsk()} disabled={S.rpBusy || !st || st === "asleep"}>{ct(S.lang, "rp_sleep_btn")}</button>
  </div>
  <div class="row small" style="margin-top:8px">
    <span class="dim">{ct(S.lang, "rp_sleep_after")}</span>
    <select value={sleepMin} onchange={(e) => S.set("runpod_sleep_min", e.currentTarget.value)} style="width:auto">
      <option value="2">2 min</option><option value="5">5 min</option><option value="10">10 min</option><option value="30">30 min</option><option value="0">{ct(S.lang, "rp_sleep_never")}</option>
    </select>
  </div>
  <label class="row small dim" style="margin-top:6px"><input type="checkbox" checked={S.settings.runpod_sleep_on_exit !== "0"} onchange={(e) => S.set("runpod_sleep_on_exit", e.currentTarget.checked ? "1" : "0")} />{ct(S.lang, "rp_sleep_exit")}</label>
  <p class="small dim" style="margin-top:6px">{ct(S.lang, "rp_sleep_note")}</p>
{/if}
