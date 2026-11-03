<script lang="ts">
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { runCommand, shortcutFor } from "../commands.svelte";
  import { isMac, shortcutLabel } from "../format";
  import Icon from "./Icon.svelte";

  const tip = (label: string, id: string) => {
    const k = shortcutFor(id);
    return k ? `${label} (${shortcutLabel(k)})` : label;
  };
</script>

<header class="bar" class:mac={isMac} data-tauri-drag-region>
  <div class="tabs" data-tauri-drag-region>
    {#each store.tabs as t (t.id)}
      <div
        class="tab"
        class:active={t.id === store.activeId}
        role="tab"
        tabindex="-1"
        aria-selected={t.id === store.activeId}
        title={t.info.path ?? t.info.name}
        onmousedown={(e) => {
          if (e.button === 1) {
            e.preventDefault();
            store.close(t);
          } else if (e.button === 0) store.activate(t.id);
        }}
      >
        {#if t.info.readonly}<span class="lock"><Icon name="lock" size={12} /></span>{/if}
        <span class="name">{t.info.name}</span>
        {#if t.compare}<span class="vs">↔ {t.compare.info.name}</span>{/if}
        <button
          class="close"
          class:dirty={t.info.modified}
          title="Close"
          onmousedown={(e) => e.stopPropagation()}
          onclick={() => store.close(t)}
        >
          <span class="dot"></span>
          <Icon name="x" size={12} stroke={2.2} />
        </button>
      </div>
    {/each}
    <button class="icon-btn new" title={tip("New file", "new")} onclick={() => runCommand("new")}>
      <Icon name="plus" size={15} />
    </button>
  </div>
  <div class="actions">
    <button class="icon-btn" title={tip("Open", "open")} onclick={() => runCommand("open")}><Icon name="folder" /></button>
    <button class="icon-btn" title={tip("Save", "save")} disabled={!store.active} onclick={() => runCommand("save")}><Icon name="save" /></button>
    <button class="icon-btn" class:active={!!store.active?.compare} title={tip("Compare with…", "compare")} disabled={!store.active} onclick={() => (store.active?.compare ? store.closeCompare() : runCommand("compare"))}><Icon name="compare" /></button>
    <button class="icon-btn" title={tip("Go to offset", "goto")} disabled={!store.active} onclick={() => runCommand("goto")}><Icon name="target" /></button>
    <button class="icon-btn" title={tip("Command palette", "palette")} onclick={() => runCommand("palette")}><Icon name="command" /></button>
    <button class="icon-btn" class:active={settings.data.sidebarOpen} title={tip("Toggle sidebar", "toggle-sidebar")} onclick={() => runCommand("toggle-sidebar")}><Icon name="sidebar" /></button>
  </div>
</header>

<style>
  .bar {
    height: 40px;
    flex: none;
    display: flex;
    align-items: stretch;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    padding: 0 6px 0 6px;
    gap: 8px;
  }
  .tabs {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    max-width: 260px;
    padding: 0 6px 0 12px;
    border-radius: 8px 8px 0 0;
    color: var(--text-2);
    cursor: default;
    flex: none;
    border: 1px solid transparent;
    border-bottom: none;
  }
  .tab:hover {
    background: var(--hover);
  }
  .tab.active {
    background: var(--hx-bg);
    color: var(--text);
    border-color: var(--border);
    margin-bottom: -1px;
    height: 33px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vs {
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
  }
  .lock {
    color: var(--warning);
    display: inline-flex;
  }
  .close {
    width: 20px;
    height: 20px;
    border: none;
    background: transparent;
    border-radius: 5px;
    display: grid;
    place-items: center;
    color: var(--muted);
    cursor: pointer;
    flex: none;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  .close :global(svg) {
    grid-area: 1 / 1;
    opacity: 0;
  }
  .tab:hover .close :global(svg),
  .tab.active .close:not(.dirty) :global(svg) {
    opacity: 1;
  }
  .dot {
    grid-area: 1 / 1;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-2);
    opacity: 0;
  }
  .close.dirty .dot {
    opacity: 1;
  }
  .tab:hover .close.dirty .dot {
    opacity: 0;
  }
  .new {
    margin: 0 0 3px 4px;
    align-self: flex-end;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }
  .icon-btn:disabled {
    opacity: 0.35;
    pointer-events: none;
  }
</style>
