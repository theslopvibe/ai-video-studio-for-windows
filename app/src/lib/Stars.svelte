<script lang="ts">
  let { n }: { n: number } = $props();
</script>

<span title="{n}/5" aria-label="{n}/5" style="letter-spacing:1px;color:var(--warn)">{"★".repeat(n)}<span style="opacity:.3">{"★".repeat(5 - n)}</span></span>
