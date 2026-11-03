<script lang="ts">
  import { store, type Tab } from "../store.svelte";
  import { hex } from "../format";
  import TreeItem from "./TreeItem.svelte";
  import VirtualList from "../components/VirtualList.svelte";

  let { tab }: { tab: Tab } = $props();

  let view = $state<"tree" | "symbols" | "regions">("tree");
  let filter = $state("");
  const f = $derived(tab.format);
  const symbols = $derived.by(() => {
    const list = f?.symbols ?? [];
    const q = filter.trim().toLowerCase();
    return q ? list.filter((s) => s.name.toLowerCase().includes(q)) : list;
  });
  const isExecutable = $derived(!!f && f.tree.length > 0);
</script>

{#if !f}
  <div class="empty">Analyzing…</div>
{:else}
  <div class="panel-section">
    <div class="kind">{f.kind}</div>
    <div class="desc">{f.description}</div>
    {#if f.entry !== null}
      <button class="entry" onclick={() => store.gotoEntry(tab)} disabled={f.entryOffset === null}>
        <span class="muted">Entry point</span>
        <span class="mono">0x{hex(f.entry)}</span>
        {#if f.entryOffset !== null}<span class="muted mono">@ 0x{hex(f.entryOffset)}</span>{/if}
      </button>
    {/if}
  </div>

  {#if isExecutable}
    <div class="tabs">
      <div class="segmented">
        <button class:on={view === "tree"} onclick={() => (view = "tree")}>Headers</button>
        <button class:on={view === "regions"} onclick={() => (view = "regions")}>Regions</button>
        <button class:on={view === "symbols"} onclick={() => (view = "symbols")}>Symbols {f.symbols.length ? `(${f.symbols.length.toLocaleString("en-US")})` : ""}</button>
      </div>
    </div>

    {#if view === "tree"}
      <div class="scroll" role="tree">
        {#each f.tree as node, i}
          <TreeItem {node} {tab} open={i < 2 || node.label === "Sections" || node.label === "Segments"} />
        {/each}
      </div>
    {:else if view === "regions"}
      <div class="scroll">
        {#each f.regions as r}
          <button
            class="region"
            onmouseenter={() => (tab.highlight = { offset: r.offset, len: r.size })}
            onmouseleave={() => (tab.highlight = null)}
            onclick={() => store.select(tab, r.offset, r.size)}
          >
            <span class="rname">{r.name}</span>
            <span class="tag">{r.kind}</span>
            {#if r.flags}<span class="mono flags">{r.flags}</span>{/if}
            <span class="mono muted range">0x{hex(r.offset)}–0x{hex(r.offset + r.size)}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="panel-section filter">
        <input placeholder="Filter symbols" bind:value={filter} onkeydown={(e) => e.stopPropagation()} spellcheck="false" />
      </div>
      {#if symbols.length === 0}
        <div class="empty">{f.symbols.length ? "No symbols match" : "No symbols in this binary (stripped?)"}</div>
      {:else}
        <VirtualList items={symbols} rowHeight={24}>
          {#snippet row(s)}
            <button
              class="sym"
              disabled={s.offset === undefined}
              title={s.offset === undefined ? "Not backed by file data" : `${s.kind} · offset 0x${hex(s.offset)}`}
              onclick={() => s.offset !== undefined && store.select(tab, s.offset, Math.max(1, s.size))}
            >
              <span class="mono addr">0x{hex(s.vaddr, 8)}</span>
              <span class="sname">{s.name}</span>
              <span class="tag">{s.kind}</span>
            </button>
          {/snippet}
        </VirtualList>
      {/if}
    {/if}
  {:else}
    <div class="empty">
      No executable structure to show.<br />
      Hexly parses ELF, PE and Mach-O (including universal binaries).
    </div>
  {/if}
{/if}

<style>
  .kind {
    font-size: 15px;
    font-weight: 600;
  }
  .desc {
    color: var(--text-2);
    margin-top: 3px;
    line-height: 1.4;
    user-select: text;
    -webkit-user-select: text;
  }
  .entry {
    margin-top: 10px;
    display: flex;
    gap: 8px;
    align-items: center;
    width: 100%;
    border: 1px solid var(--border-strong);
    background: var(--panel-2);
    border-radius: 6px;
    padding: 6px 10px;
    cursor: pointer;
    font-size: 12.5px;
  }
  .entry:hover {
    border-color: var(--accent);
  }
  .tabs {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .tabs .segmented {
    width: 100%;
    display: flex;
  }
  .tabs .segmented button {
    flex: 1;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 4px 0 16px;
  }
  .region,
  .sym {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 24px;
    padding: 2px 12px;
    border: none;
    background: transparent;
    cursor: pointer;
    font-size: 12.5px;
    text-align: left;
  }
  .region:hover,
  .sym:hover:not(:disabled) {
    background: var(--hover);
  }
  .sym:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .rname,
  .sname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .sname {
    flex: 1;
  }
  .tag {
    font-size: 10.5px;
    color: var(--muted);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 0 4px;
    flex: none;
  }
  .flags {
    font-size: 11px;
    color: var(--text-2);
  }
  .range {
    margin-left: auto;
    font-size: 11px;
    white-space: nowrap;
  }
  .addr {
    font-size: 11.5px;
    color: var(--text-2);
    flex: none;
  }
  .filter input {
    width: 100%;
  }
</style>
