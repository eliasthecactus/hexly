<script lang="ts">
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { setPanel, shortcutFor } from "../commands.svelte";
  import { shortcutLabel } from "../format";
  import Icon from "./Icon.svelte";
  import Inspector from "../panels/Inspector.svelte";
  import Structure from "../panels/Structure.svelte";
  import Search from "../panels/Search.svelte";
  import Strings from "../panels/Strings.svelte";
  import Disasm from "../panels/Disasm.svelte";
  import Bookmarks from "../panels/Bookmarks.svelte";
  import Hashes from "../panels/Hashes.svelte";

  const PANELS = [
    { id: "inspector", label: "Data Inspector", icon: "inspector" },
    { id: "structure", label: "Structure", icon: "structure" },
    { id: "search", label: "Search", icon: "search" },
    { id: "strings", label: "Strings", icon: "strings" },
    { id: "disasm", label: "Disassembly", icon: "cpu" },
    { id: "bookmarks", label: "Bookmarks", icon: "bookmark" },
    { id: "hashes", label: "Hashes", icon: "hash" },
  ];

  const tab = $derived(store.active);
  const panel = $derived(PANELS.find((p) => p.id === settings.data.sidebarPanel) ?? PANELS[0]);

  function startResize(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = settings.data.sidebarWidth;
    const move = (ev: MouseEvent) => {
      settings.data.sidebarWidth = Math.max(260, Math.min(720, startW + (startX - ev.clientX)));
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", () => {
      window.removeEventListener("mousemove", move);
      settings.save();
    }, { once: true });
  }
</script>

{#if settings.data.sidebarOpen}
  <aside class="sidebar" style:width="{settings.data.sidebarWidth}px">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="resizer" onmousedown={startResize}></div>
    <div class="title">{panel.label}</div>
    <div class="content">
      {#if !tab}
        <div class="empty">Open a file to use the {panel.label.toLowerCase()}.</div>
      {:else}
        {#key tab.id}
          {#if panel.id === "inspector"}<Inspector {tab} />
          {:else if panel.id === "structure"}<Structure {tab} />
          {:else if panel.id === "search"}<Search {tab} />
          {:else if panel.id === "strings"}<Strings {tab} />
          {:else if panel.id === "disasm"}<Disasm {tab} />
          {:else if panel.id === "bookmarks"}<Bookmarks {tab} />
          {:else if panel.id === "hashes"}<Hashes {tab} />
          {/if}
        {/key}
      {/if}
    </div>
  </aside>
{/if}
<nav class="rail">
  {#each PANELS as p, i}
    <button
      class="icon-btn"
      class:active={settings.data.sidebarOpen && settings.data.sidebarPanel === p.id}
      title="{p.label} ({shortcutLabel(shortcutFor(`panel-${p.id}`) ?? `Mod+${i + 1}`)})"
      onclick={() => setPanel(p.id)}
    >
      <Icon name={p.icon} size={17} />
    </button>
  {/each}
  <span class="grow"></span>
  <button class="icon-btn" title="Settings" onclick={() => (store.dialog = "settings")}><Icon name="sliders" size={17} /></button>
</nav>

<style>
  .sidebar {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--border);
    min-width: 0;
  }
  .resizer {
    position: absolute;
    left: -3px;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 3;
  }
  .resizer:hover {
    background: var(--accent-soft);
  }
  .title {
    height: 34px;
    flex: none;
    display: flex;
    align-items: center;
    padding: 0 12px;
    font-weight: 600;
    font-size: 12.5px;
    border-bottom: 1px solid var(--border);
  }
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: auto;
  }
  .rail {
    width: 44px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--panel);
    border-left: 1px solid var(--border);
  }
  .rail .icon-btn {
    width: 32px;
    height: 32px;
  }
  .grow {
    flex: 1;
  }
</style>
