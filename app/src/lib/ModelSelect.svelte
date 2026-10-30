<script lang="ts">
  import { api } from "./api";
  import { S } from "./store.svelte";
  import { ct } from "./chatStrings";

  // A model picker: the provider's suggested and live models, plus "Custom…" for anything else.
  let { provider, value, onchange, id = "", width = "" }: { provider: string; value: string; onchange: (v: string) => void; id?: string; width?: string } = $props();

  let models = $state<string[]>([]);
  let custom = $state(false);

  $effect(() => {
    const p = provider;
    custom = false;
    api.chatModels(p).then((m) => { if (p === provider) models = m; }).catch(() => { models = []; });
  });

  const options = $derived(value && !models.includes(value) ? [value, ...models] : models);
  const style = $derived(width ? `width:${width}` : "");
</script>

{#if custom}
  <input {id} {style} {value} placeholder={S.t("model")} onchange={(e) => { const v = e.currentTarget.value.trim(); if (v) onchange(v); }} />
{:else}
  <select {id} {style} {value} title={S.t("model")} onchange={(e) => { const v = e.currentTarget.value; if (v === "__custom__") { custom = true; return; } onchange(v); }}>
    {#each options as m}<option value={m}>{m}</option>{/each}
    <option value="__custom__">{ct(S.lang, "custom_model")}</option>
  </select>
{/if}
