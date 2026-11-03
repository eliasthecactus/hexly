<script lang="ts">
  import { api, errorText } from "../api";
  import { store, type Tab } from "../store.svelte";
  import { hex } from "../format";
  import type { FoundString } from "../types";
  import VirtualList from "../components/VirtualList.svelte";

  let { tab }: { tab: Tab } = $props();

  let minLen = $state(5);
  let ascii = $state(true);
  let utf16 = $state(true);
  let filter = $state("");
  let results = $state.raw<FoundString[]>([]);
  let truncated = $state(false);
  let running = $state<{ cancel: () => void } | null>(null);
  let progress = $state(0);
  let scannedFor = $state<number | null>(null);
  let error = $state("");

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return q ? results.filter((s) => s.text.toLowerCase().includes(q)) : results;
  });

  async function scan() {
    error = "";
    const t = api.findStrings(tab.id, { minLen, ascii, utf16, limit: 500_000 }, (p) => (progress = p));
    running = t;
    progress = 0;
    try {
      const r = await t.promise;
      results = r.strings;
      truncated = r.truncated;
      scannedFor = tab.info.stateId;
    } catch (e) {
      error = errorText(e);
    } finally {
      running = null;
    }
  }

</script>

<div class="panel-section">
  <div class="row opts">
    <label>Min length <input type="number" min="2" max="256" bind:value={minLen} onkeydown={(e) => e.stopPropagation()} /></label>
    <label class="check"><input type="checkbox" bind:checked={ascii} /> ASCII</label>
    <label class="check"><input type="checkbox" bind:checked={utf16} /> UTF-16</label>
    <span class="grow"></span>
    {#if running}
      <button class="btn" onclick={() => running?.cancel()}>Cancel {Math.round(progress * 100)}%</button>
    {:else}
      <button class="btn primary" disabled={!ascii && !utf16} onclick={scan}>{results.length ? "Rescan" : "Scan"}</button>
    {/if}
  </div>
  {#if results.length}
    <input class="filter" placeholder="Filter strings" bind:value={filter} onkeydown={(e) => e.stopPropagation()} spellcheck="false" />
  {/if}
  {#if error}<div class="err">{error}</div>{/if}
</div>

{#if results.length}
  <div class="count">
    {shown.length.toLocaleString("en-US")}{filter ? ` of ${results.length.toLocaleString("en-US")}` : ""}{truncated ? "+" : ""} strings
    {#if scannedFor !== tab.info.stateId}<span class="stale">file changed, rescan to update</span>{/if}
  </div>
  <VirtualList items={shown} rowHeight={24}>
    {#snippet row(s)}
      <button
        class="str"
        onclick={() => store.select(tab, s.offset, s.len)}
        onmouseenter={() => (tab.highlight = { offset: s.offset, len: s.len })}
        onmouseleave={() => (tab.highlight = null)}
        title={s.text}
      >
        <span class="mono off">0x{hex(s.offset, 8)}</span>
        {#if s.encoding === "utf16le"}<span class="tag">U16</span>{/if}
        <span class="text mono">{s.text}</span>
      </button>
    {/snippet}
  </VirtualList>
{:else if !running}
  <div class="empty">Find printable strings in the file: paths, messages, URLs, symbol names and other text hidden in binaries.</div>
{/if}

<style>
  .opts {
    flex-wrap: wrap;
  }
  .opts label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
  }
  .opts input[type="number"] {
    width: 56px;
  }
  .filter {
    width: 100%;
    margin-top: 8px;
  }
  .count {
    padding: 6px 12px;
    font-size: 12px;
    color: var(--text-2);
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: 8px;
  }
  .stale {
    color: var(--warning);
  }
  .str {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 24px;
    padding: 0 12px;
    border: none;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }
  .str:hover {
    background: var(--hover);
  }
  .off {
    font-size: 11.5px;
    color: var(--muted);
    flex: none;
  }
  .text {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: 10px;
    color: var(--accent-2);
    border: 1px solid var(--accent-soft);
    border-radius: 4px;
    padding: 0 3px;
    flex: none;
  }
  .err {
    color: var(--danger);
    margin-top: 8px;
    font-size: 12px;
  }
</style>
