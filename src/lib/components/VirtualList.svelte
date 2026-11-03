<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  let { items, rowHeight = 26, row }: { items: T[]; rowHeight?: number; row: Snippet<[T, number]> } = $props();

  let scrollTop = $state(0);
  let viewH = $state(400);

  const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 8));
  const last = $derived(Math.min(items.length, Math.ceil((scrollTop + viewH) / rowHeight) + 8));
  const visible = $derived(items.slice(first, last));
</script>

<div class="vlist" bind:clientHeight={viewH} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="spacer" style:height="{items.length * rowHeight}px">
    <div class="rows" style:transform="translateY({first * rowHeight}px)">
      {#each visible as item, i (first + i)}
        <div class="vrow" style:height="{rowHeight}px">{@render row(item, first + i)}</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .vlist {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .spacer {
    position: relative;
  }
  .rows {
    position: absolute;
    inset: 0 0 auto 0;
  }
  .vrow {
    overflow: hidden;
  }
</style>
