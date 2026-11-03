<script lang="ts">
  import { store, type Tab } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { fmtCount, fmtSize, hex } from "../format";

  let { tab }: { tab: Tab | null } = $props();

  const sel = $derived(tab?.selection);
  const region = $derived(tab ? tab.regionAt(tab.cursor) : null);
  const va = $derived(tab ? tab.vaOf(tab.cursor) : null);
  const fmtNum = (n: number) => (settings.data.offsetBase === "hex" ? `0x${hex(n)}` : n.toLocaleString("en-US"));
</script>

<footer class="status">
  {#if tab}
    <button class="item" title="Go to offset" onclick={() => (store.dialog = "goto")}>
      <span class="k">Offset</span> <span class="mono">{fmtNum(tab.cursor)}</span>
    </button>
    {#if va !== null}
      <span class="item"><span class="k">VA</span> <span class="mono">0x{hex(va)}</span></span>
    {/if}
    {#if region}
      <span class="item region" title="{region.kind}: {region.name}">{region.name}</span>
    {/if}
    {#if sel && sel.len > 0}
      <span class="item"><span class="k">Selected</span> <span class="mono">{fmtCount(sel.len)}</span></span>
    {/if}
    {#if tab.compare}
      <span class="item diff">
        {#if tab.compare.diffCount === null}comparing…{:else if tab.compare.diffCount === 0}files are identical{:else}{tab.compare.diffCount.toLocaleString("en-US")} bytes differ{/if}
      </span>
    {/if}
    <span class="grow"></span>
    {#if store.busy}
      <span class="item busy">
        <span class="spinner"></span>
        {store.busy.label}
        {#if store.busy.progress >= 0}<span class="mono">{Math.round(store.busy.progress * 100)}%</span>{/if}
        {#if store.busy.cancel}<button class="link" onclick={() => store.busy?.cancel?.()}>Cancel</button>{/if}
      </span>
    {/if}
    <span class="item muted" title={tab.format?.description ?? ""}>{tab.format?.kind ?? "…"}</span>
    <span class="item"><span class="mono">{fmtSize(tab.info.size)}</span></span>
    <button class="item toggle" title="Endianness for the inspector and number search" onclick={() => settings.set("littleEndian", !settings.data.littleEndian)}>
      {settings.data.littleEndian ? "LE" : "BE"}
    </button>
    <button class="item toggle" title="Text column encoding" onclick={() => {
      const order = ["ascii", "latin1", "ebcdic"] as const;
      settings.set("textMode", order[(order.indexOf(settings.data.textMode) + 1) % order.length]);
    }}>
      {settings.data.textMode === "ascii" ? "ASCII" : settings.data.textMode === "latin1" ? "Latin-1" : "EBCDIC"}
    </button>
    <button
      class="item toggle mode"
      class:ins={tab.insertMode}
      title={tab.insertMode ? "Insert mode: typing inserts bytes and changes the file size" : "Overwrite mode: the file size stays fixed"}
      onclick={() => store.toggleInsert(tab)}
    >
      {tab.insertMode ? "INS" : "OVR"}
    </button>
    {#if tab.info.readonly}<span class="item ro">read-only</span>{/if}
  {:else}
    <span class="item muted">No file open</span>
    <span class="grow"></span>
  {/if}
</footer>

<style>
  .status {
    height: 26px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px;
    background: var(--panel);
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 7px;
    height: 20px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 12px;
  }
  button.item {
    cursor: pointer;
  }
  button.item:hover {
    background: var(--hover);
    color: var(--text);
  }
  .k {
    color: var(--muted);
  }
  .region {
    color: var(--accent-2);
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    display: inline-block;
    line-height: 20px;
  }
  .toggle {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.02em;
  }
  .mode.ins {
    color: var(--warning);
  }
  .ro {
    color: var(--warning);
  }
  .diff {
    color: var(--danger);
  }
  .busy {
    color: var(--text);
  }
  .link {
    border: none;
    background: none;
    color: var(--accent-2);
    cursor: pointer;
    padding: 0;
    font-size: 12px;
  }
  .spinner {
    width: 11px;
    height: 11px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent-2);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
