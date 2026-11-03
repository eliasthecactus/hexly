<script lang="ts">
  import { api, errorText } from "../api";
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { fmtCount } from "../format";
  import type { CopyFormat } from "../types";
  import Modal from "./Modal.svelte";

  const tab = store.active!;
  const { start, len } = tab.target;
  const FORMATS: [CopyFormat, string][] = [
    ["hex", "Hex (spaced)"], ["hexCompact", "Hex (compact)"], ["text", "Text"], ["escapedString", "Escaped string"],
    ["c", "C array"], ["rust", "Rust array"], ["python", "Python bytes"], ["go", "Go slice"], ["java", "Java array"],
    ["base64", "Base64"],
  ];
  let format = $state<CopyFormat>("c");
  let preview = $state("");
  let error = $state("");

  $effect(() => {
    const f = format;
    const upper = settings.data.uppercase;
    api.formatBytes(tab.id, start, Math.min(len, 256), f, upper)
      .then((p) => (preview = p + (len > 256 ? "\n…" : "")))
      .catch((e) => (error = errorText(e)));
  });

  async function copy() {
    try {
      await api.copyBytes(tab.id, start, len, format, settings.data.uppercase);
      store.toast(`Copied ${fmtCount(len)} bytes`, "success", undefined, 1500);
      store.dialog = null;
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<Modal title="Copy {fmtCount(len)} bytes as" width={620}>
  {#if len === 0}
    <p class="muted">Nothing to copy.</p>
  {:else}
    <div class="wrap">
      <div class="list" role="listbox">
        {#each FORMATS as [id, label]}
          <button class:on={format === id} role="option" aria-selected={format === id} onclick={() => (format = id)} ondblclick={copy}>{label}</button>
        {/each}
      </div>
      <pre class="preview mono">{preview}</pre>
    </div>
    {#if error}<p class="err">{error}</p>{/if}
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => (store.dialog = null)}>Cancel</button>
    <button class="btn primary" disabled={len === 0} onclick={copy}>Copy</button>
  {/snippet}
</Modal>

<style>
  .wrap {
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 12px;
    height: 300px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .list button {
    text-align: left;
    padding: 6px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    color: var(--text-2);
  }
  .list button:hover {
    background: var(--hover);
  }
  .list button.on {
    background: var(--accent-soft);
    color: var(--text);
  }
  .preview {
    margin: 0;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px;
    overflow: auto;
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
    -webkit-user-select: text;
  }
  .err {
    color: var(--danger);
  }
</style>
