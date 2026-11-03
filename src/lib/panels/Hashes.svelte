<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { api, errorText } from "../api";
  import { store, type Tab } from "../store.svelte";
  import { fmtCount, hex } from "../format";
  import type { HashResult } from "../types";
  import Icon from "../components/Icon.svelte";

  let { tab }: { tab: Tab } = $props();

  let results = $state<HashResult[]>([]);
  let scope = $state("");
  let running = $state<{ cancel: () => void } | null>(null);
  let progress = $state(0);
  let error = $state("");

  const sel = $derived(tab.selection);

  async function run(whole: boolean) {
    const offset = whole ? 0 : sel.start;
    const len = whole ? tab.info.size : sel.len;
    error = "";
    const t = api.hashes(tab.id, offset, len, (p) => (progress = p));
    running = t;
    progress = 0;
    try {
      const r = await t.promise;
      if (r.length) {
        results = r;
        scope = whole ? `Whole file · ${fmtCount(len)} bytes` : `0x${hex(offset)}–0x${hex(offset + len)} · ${fmtCount(len)} bytes`;
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      running = null;
    }
  }

  async function copy(v: string) {
    await writeText(v);
    store.toast("Copied", "success", undefined, 1200);
  }
</script>

<div class="panel-section row">
  {#if running}
    <span class="grow muted">Hashing… {Math.round(progress * 100)}%</span>
    <button class="btn" onclick={() => running?.cancel()}>Cancel</button>
  {:else}
    <button class="btn primary grow" onclick={() => run(true)}>Hash whole file</button>
    <button class="btn grow" disabled={sel.len === 0} onclick={() => run(false)}>Hash selection</button>
  {/if}
</div>
{#if error}<div class="empty err">{error}</div>{/if}

{#if results.length}
  <div class="scope">{scope}</div>
  <div class="list">
    {#each results as h}
      <div class="h">
        <div class="name">{h.name}</div>
        <div class="row">
          <span class="v mono grow">{h.value}</span>
          <button class="icon-btn" title="Copy" onclick={() => copy(h.value)}><Icon name="copy" size={14} /></button>
        </div>
      </div>
    {/each}
  </div>
{:else if !running}
  <div class="empty">CRC32, Adler-32, MD5, SHA-1, SHA-256, SHA-512 and xxHash for the whole file or the selection.</div>
{/if}

<style>
  .scope {
    padding: 8px 12px;
    font-size: 12px;
    color: var(--text-2);
    border-bottom: 1px solid var(--border);
  }
  .list {
    overflow: auto;
    flex: 1;
    padding: 4px 0 12px;
  }
  .h {
    padding: 6px 12px;
  }
  .name {
    font-size: 11px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .v {
    font-size: 12px;
    word-break: break-all;
    user-select: text;
    -webkit-user-select: text;
  }
  .err {
    color: var(--danger);
  }
</style>
