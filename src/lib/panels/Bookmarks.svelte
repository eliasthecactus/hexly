<script lang="ts">
  import { store, type Tab } from "../store.svelte";
  import { fmtCount, hex, shortcutLabel } from "../format";
  import { shortcutFor } from "../commands.svelte";
  import Icon from "../components/Icon.svelte";

  let { tab }: { tab: Tab } = $props();
  const COLORS = ["#f59e0b", "#22c55e", "#3b82f6", "#ec4899", "#a855f7", "#14b8a6", "#ef4444"];

  function remove(id: string) {
    tab.bookmarks = tab.bookmarks.filter((b) => b.id !== id);
    store.persistBookmarks(tab);
  }

  function cycleColor(i: number) {
    const b = tab.bookmarks[i];
    b.color = COLORS[(COLORS.indexOf(b.color) + 1) % COLORS.length];
    store.persistBookmarks(tab);
  }
</script>

<div class="panel-section row">
  <span class="grow muted small">
    {#if tab.info.path}Saved per file on this computer{:else}Save the file to keep bookmarks{/if}
  </span>
  <button class="btn" onclick={() => store.toggleBookmark(tab)}>
    <Icon name="bookmark" size={14} /> Add {shortcutLabel(shortcutFor("toggle-bookmark") ?? "")}
  </button>
</div>

{#if tab.bookmarks.length === 0}
  <div class="empty">
    Mark interesting offsets and ranges. Select bytes and press {shortcutLabel(shortcutFor("toggle-bookmark") ?? "")}.
    Jump between bookmarks with F2 / Shift+F2.
  </div>
{:else}
  <div class="list">
    {#each tab.bookmarks as b, i (b.id)}
      <div
        class="bm"
        role="listitem"
        onmouseenter={() => (tab.highlight = { offset: b.offset, len: b.len })}
        onmouseleave={() => (tab.highlight = null)}
      >
        <button class="swatch" style:background={b.color} title="Change color" onclick={() => cycleColor(i)} aria-label="Change color"></button>
        <div class="grow">
          <input
            class="label"
            bind:value={b.label}
            onchange={() => store.persistBookmarks(tab)}
            onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
            spellcheck="false"
          />
          <button class="where mono" onclick={() => store.select(tab, b.offset, b.len)}>
            0x{hex(b.offset)} · {fmtCount(b.len)}
          </button>
        </div>
        <button class="icon-btn" title="Remove" onclick={() => remove(b.id)}><Icon name="trash" size={14} /></button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .small {
    font-size: 12px;
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 6px 0;
  }
  .bm {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 6px 12px;
  }
  .bm:hover {
    background: var(--hover);
  }
  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    border: none;
    cursor: pointer;
    flex: none;
  }
  .label {
    width: 100%;
    border: 1px solid transparent;
    background: transparent;
    padding: 1px 4px;
    margin-left: -5px;
    font-size: 12.5px;
  }
  .label:hover {
    border-color: var(--border-strong);
  }
  .where {
    border: none;
    background: none;
    color: var(--text-2);
    font-size: 11.5px;
    padding: 0;
    cursor: pointer;
  }
  .where:hover {
    color: var(--accent-2);
  }
</style>
