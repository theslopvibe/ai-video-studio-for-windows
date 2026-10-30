<script lang="ts">
  import { S } from "./store.svelte";
  import { GUIDE } from "./guide";
  import { loc } from "./translations";

  let open = $state("start");
  const go = (page: "generate" | "models" | "settings" | "gpu") => (S.page = page);
</script>

<div class="page">
  <h1>{S.t("nav_guide")}</h1>
  <p class="dim">{S.t("guide_intro")}</p>

  {#each GUIDE as g}
    {@const base = S.lang === "el" ? g.el : g.en}
    {@const c = S.lang === "el" || S.lang === "en" ? base : { t: loc(S.lang, `g.${g.id}.t`, base.t), steps: base.steps.map((s, i) => loc(S.lang, `g.${g.id}.s${i}`, s)), note: base.note ? loc(S.lang, `g.${g.id}.note`, base.note) : undefined }}
    <div class="card">
      <button class="ghost" style="width:100%;text-align:left;font-size:15px" onclick={() => (open = open === g.id ? "" : g.id)}>
        <b>{open === g.id ? "▾" : "▸"} {g.icon}. {c.t}</b>
      </button>
      {#if open === g.id}
        <ol style="margin:10px 0 0;padding-left:22px;line-height:1.6">
          {#each c.steps as st}<li>{st}</li>{/each}
        </ol>
        {#if c.note}<div class="msg">{c.note}</div>{/if}
        <div class="row" style="margin-top:10px">
          {#if g.id === "start"}
            <button onclick={() => go("settings")}>{S.t("nav_settings")}</button>
            <button onclick={() => go("gpu")}>{S.t("nav_gpu")}</button>
            <button onclick={() => go("models")}>{S.t("nav_models")}</button>
          {/if}
          {#if g.id === "t2v" || g.id === "i2v"}<button class="primary" onclick={() => go("generate")}>{S.t("nav_generate")}</button>{/if}
          {#if g.id === "addmodel"}<button class="primary" onclick={() => go("models")}>{S.t("nav_models")}</button>{/if}
        </div>
      {/if}
    </div>
  {/each}
</div>
