<script lang="ts">
  import { store } from "../store.svelte";
  import { fmtCount, hex, parseHex } from "../format";
  import Modal from "./Modal.svelte";

  /** "fill" overwrites/fills the selection, "insert" inserts new bytes at the cursor. */
  let { mode }: { mode: "fill" | "insert" } = $props();

  const tab = store.active!;
  const sel = tab.selection;
  let pattern = $state("00");
  // svelte-ignore state_referenced_locally
  let count = $state(mode === "fill" && sel.len > 0 ? String(sel.len) : "16");
  let input: HTMLInputElement;

  $effect(() => {
    input?.select();
  });

  const bytes = $derived(parseHex(pattern));
  const n = $derived.by(() => {
    const t = count.trim().toLowerCase();
    const v = t.startsWith("0x") ? parseInt(t.slice(2), 16) : parseInt(t, 10);
    return Number.isFinite(v) && v > 0 ? v : null;
  });
  const at = sel.len > 0 ? sel.start : tab.cursor;
  // svelte-ignore state_referenced_locally
  const insert = mode === "insert";

  async function apply() {
    if (!bytes || !n) return;
    if (tab.info.readonly) return store.toast("This file is open read-only.");
    if (!insert && !tab.insertMode && at + n > tab.info.size) {
      store.toast("The fill would extend the file. Switch to insert mode or select fewer bytes.");
      return;
    }
    const ok = await store.fill(tab, pattern, n, insert);
    if (ok) store.dialog = null;
  }
</script>

<Modal title={insert ? "Insert bytes" : "Fill selection"} width={420}>
  <div class="grid">
    <label for="fill-pattern">Pattern (hex)</label>
    <input id="fill-pattern" bind:this={input} class="mono" bind:value={pattern} spellcheck="false"
      onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") apply(); if (e.key === "Escape") store.dialog = null; }} />
    <label for="fill-count">Length</label>
    <input id="fill-count" class="mono" bind:value={count} spellcheck="false"
      onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") apply(); if (e.key === "Escape") store.dialog = null; }} />
  </div>
  <p class="hint">
    {#if !bytes}<span class="err">Enter hex bytes, e.g. 90 or DE AD BE EF</span>
    {:else if !n}<span class="err">Enter a length (decimal or 0x…)</span>
    {:else}
      {insert ? "Inserts" : "Writes"} {fmtCount(n)} bytes at 0x{hex(at)}{insert ? ", shifting the rest of the file" : ""}.
      Undo with one step.
    {/if}
  </p>
  {#snippet footer()}
    <button class="btn" onclick={() => (store.dialog = null)}>Cancel</button>
    <button class="btn primary" disabled={!bytes || !n} onclick={apply}>{insert ? "Insert" : "Fill"}</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 10px;
    align-items: center;
  }
  label {
    color: var(--text-2);
  }
  .hint {
    margin: 12px 0 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .err {
    color: var(--danger);
  }
</style>
