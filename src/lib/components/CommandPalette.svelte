<script lang="ts">
  import { commands, runCommand } from "../commands.svelte";
  import { shortcutLabel, basename } from "../format";
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import Modal from "./Modal.svelte";

  let query = $state("");
  let index = $state(0);
  let input: HTMLInputElement;
  let list: HTMLDivElement;

  $effect(() => {
    input?.focus();
  });

  interface Item {
    id: string;
    label: string;
    hint?: string;
    group: string;
    run: () => void;
  }

  function score(label: string, q: string): number {
    const l = label.toLowerCase();
    if (!q) return 1;
    const i = l.indexOf(q);
    if (i >= 0) return 100 - i;
    // fuzzy: every char in order
    let pos = 0;
    for (const ch of q) {
      pos = l.indexOf(ch, pos);
      if (pos < 0) return 0;
      pos++;
    }
    return 10;
  }

  const items = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const all: Item[] = commands
      .filter((c) => c.id !== "palette" && (!c.needsDoc || store.active))
      .map((c) => ({
        id: c.id, label: c.label, group: c.group, hint: c.keys ? shortcutLabel(c.keys[0]) : undefined,
        run: () => runCommand(c.id),
      }));
    for (const t of store.tabs) {
      all.push({ id: `tab-${t.id}`, label: `Switch to ${t.info.name}`, group: "Tabs", run: () => store.activate(t.id) });
    }
    for (const p of settings.data.recent) {
      all.push({ id: `recent-${p}`, label: `Open recent: ${basename(p)}`, hint: p, group: "Recent", run: () => store.open(p) });
    }
    return all
      .map((it) => ({ it, s: score(`${it.group} ${it.label}`, q) + score(it.label, q) * 2 }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 60)
      .map((x) => x.it);
  });

  $effect(() => {
    void query;
    index = 0;
  });

  function pick(it: Item | undefined) {
    if (!it) return;
    store.dialog = null;
    queueMicrotask(() => {
      (document.querySelector("canvas[data-hexview='main']") as HTMLElement | null)?.focus();
      it.run();
    });
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(items.length - 1, index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(0, index - 1);
    } else if (e.key === "Enter") {
      pick(items[index]);
    } else if (e.key === "Escape") {
      store.dialog = null;
    }
    queueMicrotask(() => list?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
  }
</script>

<Modal width={560} top>
  <input bind:this={input} class="q" placeholder="Type a command, tab or recent file…" bind:value={query} onkeydown={onKey} spellcheck="false" />
  <div class="list" bind:this={list}>
    {#each items as it, i (it.id)}
      <button class="it" class:on={i === index} onmousemove={() => (index = i)} onclick={() => pick(it)}>
        <span class="group">{it.group}</span>
        <span class="label">{it.label}</span>
        {#if it.hint}<span class="hint" class:kbd={it.group !== "Recent"}>{it.hint}</span>{/if}
      </button>
    {:else}
      <div class="none">No matching commands</div>
    {/each}
  </div>
</Modal>

<style>
  .q {
    width: 100%;
    font-size: 15px;
    padding: 9px 12px;
  }
  .list {
    margin-top: 10px;
    max-height: 52vh;
    overflow: auto;
  }
  .it {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }
  .it.on {
    background: var(--accent-soft);
  }
  .group {
    width: 56px;
    flex: none;
    color: var(--muted);
    font-size: 11.5px;
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    color: var(--muted);
    font-size: 11px;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    padding: 16px;
    text-align: center;
    color: var(--muted);
  }
</style>
