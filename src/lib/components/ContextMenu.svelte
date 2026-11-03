<script lang="ts">
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import { runCommand, shortcutFor } from "../commands.svelte";
  import { store } from "../store.svelte";
  import { isMac, shortcutLabel } from "../format";
  import { setPanel } from "../commands.svelte";

  let { x, y, onclose }: { x: number; y: number; onclose: () => void } = $props();
  const tab = store.active!;
  const hasSel = tab.selection.len > 0;
  const mod = isMac ? "⌘" : "Ctrl+";

  type Entry = { label: string; hint?: string; run: () => unknown; disabled?: boolean } | "-";
  const k = (id: string) => {
    const s = shortcutFor(id);
    return s ? shortcutLabel(s) : undefined;
  };

  const entries: Entry[] = [
    { label: "Copy", hint: `${mod}C`, run: () => store.copy(tab) },
    { label: "Copy as Hex", run: () => store.copy(tab, "hex") },
    { label: "Copy as Text", run: () => store.copy(tab, "text") },
    { label: "Copy As…", hint: k("copy-as"), run: () => runCommand("copy-as") },
    {
      label: "Paste", hint: `${mod}V`, disabled: tab.info.readonly,
      run: async () => {
        const t = await readText().catch(() => "");
        if (t) await store.paste(tab, t);
      },
    },
    "-",
    { label: hasSel ? "Fill Selection…" : "Fill…", hint: k("fill"), run: () => runCommand("fill"), disabled: tab.info.readonly },
    { label: "Insert Bytes…", hint: k("insert-bytes"), run: () => runCommand("insert-bytes"), disabled: tab.info.readonly },
    { label: "Export Selection…", run: () => store.exportSelection(tab), disabled: !hasSel },
    "-",
    { label: "Toggle Bookmark", hint: k("toggle-bookmark"), run: () => store.toggleBookmark(tab) },
    { label: "Follow Pointer (64-bit)", hint: k("follow-pointer"), run: () => store.followPointer(tab, 8) },
    { label: "Follow Pointer (32-bit)", hint: k("follow-pointer-32"), run: () => store.followPointer(tab, 4) },
    { label: "Disassemble Here", run: () => setPanel("disasm", false) },
    { label: "Hash Selection", run: () => setPanel("hashes", false), disabled: !hasSel },
  ];

  let el: HTMLDivElement;
  let pos = $state({ left: -9999, top: -9999 });

  $effect(() => {
    const r = el.getBoundingClientRect();
    pos = {
      left: Math.min(x, window.innerWidth - r.width - 8),
      top: Math.min(y, window.innerHeight - r.height - 8),
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="layer" onmousedown={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}>
  <div class="menu" bind:this={el} style:left="{pos.left}px" style:top="{pos.top}px" onmousedown={(e) => e.stopPropagation()} role="menu" tabindex="-1">
    {#each entries as e}
      {#if e === "-"}
        <div class="sep"></div>
      {:else}
        <button
          class="mi"
          role="menuitem"
          disabled={e.disabled}
          onclick={() => {
            onclose();
            Promise.resolve(e.run()).catch((err) => store.error(err));
          }}
        >
          <span>{e.label}</span>
          {#if e.hint}<span class="hint">{e.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
</div>

<style>
  .layer {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    min-width: 230px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    box-shadow: var(--shadow);
    padding: 4px;
  }
  .mi {
    display: flex;
    justify-content: space-between;
    gap: 20px;
    width: 100%;
    padding: 5px 10px;
    border: none;
    border-radius: 5px;
    background: transparent;
    cursor: pointer;
    text-align: left;
    font-size: 12.5px;
  }
  .mi:hover:not(:disabled) {
    background: var(--accent);
    color: #fff;
  }
  .mi:hover:not(:disabled) .hint {
    color: rgba(255, 255, 255, 0.8);
  }
  .mi:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .hint {
    color: var(--muted);
    font-size: 11.5px;
  }
  .sep {
    height: 1px;
    background: var(--border);
    margin: 4px 6px;
  }
</style>
