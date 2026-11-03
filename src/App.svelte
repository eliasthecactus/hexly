<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api } from "./lib/api";
  import { store } from "./lib/store.svelte";
  import { settings } from "./lib/settings.svelte";
  import { handleShortcut, quit, runCommand } from "./lib/commands.svelte";
  import { checkForUpdates } from "./lib/updater";
  import TabBar from "./lib/components/TabBar.svelte";
  import HexView from "./lib/components/HexView.svelte";
  import Minimap from "./lib/components/Minimap.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import Welcome from "./lib/components/Welcome.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import GotoDialog from "./lib/components/GotoDialog.svelte";
  import FillDialog from "./lib/components/FillDialog.svelte";
  import CopyAsDialog from "./lib/components/CopyAsDialog.svelte";
  import SettingsDialog from "./lib/components/SettingsDialog.svelte";
  import ShortcutsDialog from "./lib/components/ShortcutsDialog.svelte";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import Icon from "./lib/components/Icon.svelte";

  const tab = $derived(store.active);
  let menu = $state<{ x: number; y: number } | null>(null);
  let systemDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);

  $effect(() => {
    const t = settings.data.theme;
    document.documentElement.dataset.theme = t === "system" ? (systemDark ? "dark" : "light") : t;
  });

  function editingText() {
    const el = document.activeElement;
    return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
  }

  async function openPending() {
    const paths = await api.takePendingOpens().catch(() => []);
    for (const p of paths) await store.open(p);
  }

  onMount(() => {
    const unlisten: Array<() => void> = [];
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onScheme = (e: MediaQueryListEvent) => (systemDark = e.matches);
    media.addEventListener("change", onScheme);

    const onKey = (e: KeyboardEvent) => {
      if (store.dialog && e.key === "Escape") {
        store.dialog = null;
        return;
      }
      handleShortcut(e);
    };
    window.addEventListener("keydown", onKey);

    const onCopy = (e: ClipboardEvent) => {
      if (editingText() || !store.active || store.dialog) return;
      e.preventDefault();
      store.copy(store.active, null, e.type === "cut");
    };
    const onPaste = (e: ClipboardEvent) => {
      if (editingText() || !store.active || store.dialog) return;
      e.preventDefault();
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (text) store.paste(store.active, text);
    };
    document.addEventListener("copy", onCopy);
    document.addEventListener("cut", onCopy);
    document.addEventListener("paste", onPaste);

    const onFocus = () => store.checkDiskChange();
    window.addEventListener("focus", onFocus);

    listen<string>("menu", (e) => runCommand(e.payload)).then((u) => unlisten.push(u));
    listen("open-files", () => openPending()).then((u) => unlisten.push(u));
    getCurrentWebview()
      .onDragDropEvent(async (e) => {
        if (e.payload.type === "enter" || e.payload.type === "over") store.dragging = true;
        else if (e.payload.type === "leave") store.dragging = false;
        else if (e.payload.type === "drop") {
          store.dragging = false;
          for (const p of e.payload.paths) await store.open(p);
        }
      })
      .then((u) => unlisten.push(u));
    getCurrentWindow()
      .onCloseRequested(async (e) => {
        e.preventDefault();
        await quit();
      })
      .then((u) => unlisten.push(u));

    openPending();
    if (settings.data.checkUpdates && !import.meta.env.DEV) setTimeout(() => checkForUpdates(false), 4000);

    return () => {
      media.removeEventListener("change", onScheme);
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("copy", onCopy);
      document.removeEventListener("cut", onCopy);
      document.removeEventListener("paste", onPaste);
      window.removeEventListener("focus", onFocus);
      unlisten.forEach((u) => u());
    };
  });
</script>

<div class="app">
  <TabBar />
  <div class="main">
    {#if tab}
      <div class="editor">
        {#if tab.compare}
          <div class="compare-bar">
            <span class="side"><span class="badge">A</span>{tab.info.name}</span>
            <span class="nav">
              <button class="btn small" onclick={() => store.jumpDifference(-1)} title="Previous difference"><Icon name="arrow-up" size={13} /></button>
              <button class="btn small" onclick={() => store.jumpDifference(1)} title="Next difference"><Icon name="arrow-down" size={13} /> Next difference</button>
            </span>
            <span class="side right">
              <span class="badge b">B</span>{tab.compare.info.name}
              <button class="icon-btn" title="Close comparison" onclick={() => store.closeCompare()}><Icon name="x" size={13} /></button>
            </span>
          </div>
        {/if}
        <div class="views">
          {#key tab.id}
            <HexView {tab} oncontext={(x, y) => (menu = { x, y })} />
            {#if tab.compare}
              <div class="split"></div>
              {#key tab.compare.info.id}
                <HexView {tab} source="compare" />
              {/key}
            {/if}
            <Minimap {tab} />
          {/key}
        </div>
      </div>
    {:else}
      <Welcome />
    {/if}
    <Sidebar />
  </div>
  <StatusBar {tab} />
</div>

{#if store.dialog === "goto" && tab}<GotoDialog />{/if}
{#if store.dialog === "fill" && tab}<FillDialog mode="fill" />{/if}
{#if store.dialog === "insert" && tab}<FillDialog mode="insert" />{/if}
{#if store.dialog === "copyAs" && tab}<CopyAsDialog />{/if}
{#if store.dialog === "settings"}<SettingsDialog />{/if}
{#if store.dialog === "shortcuts"}<ShortcutsDialog />{/if}
{#if store.dialog === "palette"}<CommandPalette />{/if}
{#if menu && tab}<ContextMenu x={menu.x} y={menu.y} onclose={() => (menu = null)} />{/if}
<Toasts />

{#if store.dragging}
  <div class="dropzone">
    <div class="dropcard"><Icon name="folder" size={28} /> Drop to open</div>
  </div>
{/if}

<style>
  .app {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .main {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .editor {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .views {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .split {
    width: 1px;
    background: var(--border-strong);
    flex: none;
  }
  .compare-bar {
    height: 32px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    font-size: 12.5px;
  }
  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
    white-space: nowrap;
  }
  .side.right {
    justify-content: flex-end;
  }
  .badge {
    font-size: 10.5px;
    font-weight: 700;
    background: var(--accent);
    color: #fff;
    border-radius: 4px;
    padding: 0 5px;
  }
  .badge.b {
    background: var(--cursor);
    color: var(--hx-cursor-text);
  }
  .nav {
    display: flex;
    gap: 4px;
  }
  .dropzone {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: rgba(8, 10, 18, 0.45);
    pointer-events: none;
  }
  .dropcard {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 22px 30px;
    border: 2px dashed var(--accent-2);
    border-radius: 14px;
    background: var(--bg-elev);
    font-size: 16px;
    box-shadow: var(--shadow);
  }
</style>
