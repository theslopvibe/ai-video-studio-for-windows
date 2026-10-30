<script lang="ts">
  import { api, type GpuOffer } from "./api";
  import { S } from "./store.svelte";

  let minVram = $state(24);
  let offers = $state<GpuOffer[]>([]);
  let sel = $state<string[]>((S.settings.gpu_selection ?? "").split(",").filter(Boolean));
  let loading = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);

  let est = $derived(S.workflows.find((w) => w.meta.id.includes("lightning"))?.meta.est_sec ?? S.workflows[0]?.meta.est_sec ?? 70);
  let chosen = $derived(offers.filter((o) => sel.includes(o.name)));
  let dph = $derived(chosen.length ? chosen.reduce((a, o) => a + o.median_price, 0) / chosen.length : S.dph);

  async function load() {
    loading = true; result = null;
    const r = await S.run(() => api.gpuOffers(minVram));
    loading = false;
    if (r) offers = r;
  }
  function toggle(n: string) { sel = sel.includes(n) ? sel.filter((x) => x !== n) : [...sel, n]; }

  async function apply() {
    if (!sel.length) return (result = { ok: false, text: S.t("gpu_pick") });
    try {
      const r = await api.applyGpus(sel, minVram, dph);
      S.settings.gpu_selection = sel.join(",");
      S.settings.dph_estimate = String(dph);
      result = { ok: true, text: S.t("gpu_applied") + "\n" + r };
    } catch (e) { result = { ok: false, text: String(e) }; }
  }
</script>

<div class="page">
  <h1>{S.t("gpu_title")}</h1>
  <p class="dim">{S.t("gpu_intro")}</p>

  <div class="card">
    <div class="row">
      <div style="width:200px"><label class="small dim" for="vram">{S.t("min_vram")}</label>
        <select id="vram" bind:value={minVram}>{#each [16, 24, 32, 40, 48, 80] as v}<option value={v}>{v}</option>{/each}</select></div>
      <button class="primary" style="margin-top:18px" onclick={load} disabled={loading}>{loading ? S.t("testing") : S.t("fetch_prices")}</button>
    </div>
  </div>

  {#if offers.length}
    <div class="card">
      <table>
        <thead><tr><th></th><th>{S.t("gpu_name")}</th><th class="num">{S.t("vram")}</th><th class="num">{S.t("price_min")}</th><th class="num">{S.t("price_med")}</th><th class="num">{S.t("per_clip")}</th><th class="num">{S.t("available")}</th></tr></thead>
        <tbody>
          {#each offers as o}
            <tr>
              <td><input type="checkbox" checked={sel.includes(o.name)} onchange={() => toggle(o.name)} /></td>
              <td>{o.name.replaceAll("_", " ")}</td>
              <td class="num">{o.vram_gb} GB</td>
              <td class="num">${o.min_price.toFixed(2)}</td>
              <td class="num">${o.median_price.toFixed(2)}</td>
              <td class="num">${((o.median_price * est) / 3600).toFixed(3)}</td>
              <td class="num">{o.offers}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="small dim" style="margin-top:8px">{S.t("gpu_est_note")}</p>
      <div class="row">
        <div class="grow small dim">{sel.length ? sel.join(", ") : S.t("gpu_pick")}</div>
        <button class="primary" onclick={apply} disabled={!sel.length}>{S.t("apply_gpus")}</button>
      </div>
      {#if result}<div class="msg {result.ok ? 'ok' : 'err'}">{result.text}</div>{/if}
    </div>
  {/if}
</div>
