<script lang="ts">
  import { api, errorText } from "../api";
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { fmtCount, hex } from "../format";
  import Modal from "./Modal.svelte";

  const tab = store.active!;
  const hasVA = (tab.format?.mappings.length ?? 0) > 0;
  let expr = $state("");
  let va = $state(false);
  let result = $state<number | null>(null);
  let error = $state("");
  let input: HTMLInputElement;

  $effect(() => {
    input?.focus();
  });

  $effect(() => {
    const e = expr.trim();
    const v = va;
    void settings.data.hexDefaultGoto;
    if (!e) {
      result = null;
      error = "";
      return;
    }
    const sel = tab.selection;
    let cancelled = false;
    api
      .evalOffset(tab.id, e, {
        cursor: tab.cursor, selStart: sel.start, selEnd: sel.end, hexDefault: settings.data.hexDefaultGoto, virtualAddress: v,
      })
      .then((r) => {
        if (cancelled) return;
        result = r;
        error = r > tab.info.size ? "Past the end of the file" : "";
      })
      .catch((err) => {
        if (cancelled) return;
        result = null;
        error = errorText(err);
      });
    return () => {
      cancelled = true;
    };
  });

  function go(extend = false) {
    if (result === null || result > tab.info.size) return;
    if (extend) {
      const from = tab.cursor;
      store.select(tab, Math.min(from, result), Math.abs(result - from) + 1);
    } else {
      store.select(tab, result, 1);
    }
    store.dialog = null;
    queueMicrotask(() => (document.querySelector("canvas[data-hexview='main']") as HTMLElement | null)?.focus());
  }
</script>

<Modal title="Go to" width={460}>
  <div class="row">
    <input
      bind:this={input}
      class="mono grow"
      bind:value={expr}
      placeholder={settings.data.hexDefaultGoto ? "1F00, 0x400 + 0x20, end - 10h, sym:main" : "4096, 0x400 + 32, end - 16, sym:main"}
      spellcheck="false"
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") go(e.shiftKey);
        if (e.key === "Escape") store.dialog = null;
      }}
    />
  </div>
  <div class="opts">
    <div class="segmented">
      <button class:on={!va} onclick={() => (va = false)}>File offset</button>
      <button class:on={va} disabled={!hasVA} title={hasVA ? "" : "Only for ELF, PE and Mach-O files"} onclick={() => (va = true)}>Virtual address</button>
    </div>
    <label class="check">
      <input type="checkbox" checked={settings.data.hexDefaultGoto} onchange={(e) => settings.set("hexDefaultGoto", e.currentTarget.checked)} />
      Plain numbers are hex
    </label>
  </div>
  <div class="out">
    {#if error}
      <span class="err">{error}</span>
    {:else if result !== null}
      <span class="mono">→ offset 0x{hex(result)}</span>
      <span class="muted">({result.toLocaleString("en-US")}){result < tab.cursor ? ` · ${fmtCount(tab.cursor - result)} back` : result > tab.cursor ? ` · ${fmtCount(result - tab.cursor)} ahead` : ""}</span>
    {:else}
      <span class="muted">Names: end, cur, sel, selend, entry, sym:&lt;name&gt;, sec:&lt;name&gt;. Shift+Enter selects up to the target.</span>
    {/if}
  </div>
</Modal>

<style>
  .opts {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 10px;
    flex-wrap: wrap;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    color: var(--text-2);
    font-size: 12px;
  }
  .segmented button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .out {
    margin-top: 12px;
    min-height: 18px;
    font-size: 12.5px;
    line-height: 1.5;
  }
  .err {
    color: var(--danger);
  }
</style>
