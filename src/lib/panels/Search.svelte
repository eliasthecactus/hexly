<script lang="ts">
  import { tick } from "svelte";
  import { api, errorText } from "../api";
  import { store, type Tab } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { hex } from "../format";
  import VirtualList from "../components/VirtualList.svelte";
  import Icon from "../components/Icon.svelte";

  let { tab }: { tab: Tab } = $props();

  const s = store.search;
  let input = $state<HTMLInputElement>();
  let truncated = $state(false);
  let error = $state("");
  let running = $state<{ cancel: () => void } | null>(null);
  let progress = $state(0);
  let searchedState = $state(-1);

  $effect(() => {
    void store.focusSearch;
    tick().then(() => {
      input?.focus();
      input?.select();
    });
  });

  const placeholder = $derived(
    s.mode === "hex" ? "48 8B ?? ?? E8   (?? = any byte, 4? = any low nibble)"
    : s.mode === "text" ? "Text to find"
    : s.mode === "regex" ? "Regular expression over bytes, e.g. (?-u)\\x7FELF"
    : "Number, e.g. 0xDEADBEEF or -12",
  );

  async function findAll() {
    error = "";
    const q = store.buildQuery();
    if (!q) return;
    const t = api.findAll(tab.id, q, 200_000, (p) => (progress = p));
    running = t;
    progress = 0;
    try {
      const r = await t.promise;
      tab.matches = r.matches;
      truncated = r.truncated;
      searchedState = tab.info.stateId;
      tab.matchIndex = r.matches.length ? 0 : -1;
      if (r.matches.length) store.select(tab, r.matches[0].offset, r.matches[0].len);
    } catch (e) {
      error = errorText(e);
    } finally {
      running = null;
    }
  }

  async function replaceOne() {
    error = "";
    const q = store.buildQuery();
    if (!q) return;
    const sel = tab.selection;
    const m = tab.matches.find((x) => x.offset === sel.start && x.len === sel.len);
    if (!m) {
      await store.findNext(false, tab);
      return;
    }
    try {
      const r = await api.replaceOne(tab.id, m.offset, m.len, q, s.replace);
      store.applyInfo(tab, r.info);
      store.moveTo(tab, m.offset + r.len);
      await store.findNext(false, tab);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function replaceAll() {
    error = "";
    const q = store.buildQuery();
    if (!q) return;
    const t = api.replaceAll(tab.id, q, s.replace, (p) => (progress = p));
    running = t;
    try {
      const r = await t.promise;
      store.applyInfo(tab, r.info);
      store.toast(r.count ? `Replaced ${r.count.toLocaleString("en-US")} matches` : "No matches", r.count ? "success" : "info");
    } catch (e) {
      error = errorText(e);
    } finally {
      running = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.metaKey || e.ctrlKey) findAll();
      else store.findNext(e.shiftKey, tab);
    }
  }

  const stale = $derived(tab.matches.length > 0 && searchedState !== tab.info.stateId);
</script>

<div class="panel-section">
  <div class="segmented modes">
    {#each [["hex", "Hex"], ["text", "Text"], ["regex", "Regex"], ["number", "Number"]] as [m, label]}
      <button class:on={s.mode === m} onclick={() => { s.mode = m as typeof s.mode; input?.focus(); }}>{label}</button>
    {/each}
  </div>

  <div class="field">
    {#if s.mode === "hex"}
      <input bind:this={input} class="mono" bind:value={s.hex} {placeholder} onkeydown={onKey} spellcheck="false" />
    {:else if s.mode === "text"}
      <input bind:this={input} bind:value={s.text} {placeholder} onkeydown={onKey} spellcheck="false" />
    {:else if s.mode === "regex"}
      <input bind:this={input} class="mono" bind:value={s.regex} {placeholder} onkeydown={onKey} spellcheck="false" />
    {:else}
      <input bind:this={input} class="mono" bind:value={s.number} {placeholder} onkeydown={onKey} spellcheck="false" />
    {/if}
  </div>

  <div class="opts">
    {#if s.mode === "text"}
      <select bind:value={s.encoding}>
        <option value="utf8">UTF-8 / ASCII</option>
        <option value="utf16le">UTF-16 LE</option>
        <option value="utf16be">UTF-16 BE</option>
        <option value="latin1">Latin-1</option>
      </select>
    {/if}
    {#if s.mode === "number"}
      <select bind:value={s.numType}>
        {#each ["u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "f32", "f64"] as t}<option value={t}>{t}</option>{/each}
      </select>
      <div class="segmented">
        <button class:on={settings.data.littleEndian} onclick={() => settings.set("littleEndian", true)}>LE</button>
        <button class:on={!settings.data.littleEndian} onclick={() => settings.set("littleEndian", false)}>BE</button>
      </div>
    {/if}
    {#if s.mode === "text" || s.mode === "regex"}
      <label class="check"><input type="checkbox" bind:checked={s.caseSensitive} /> Match case</label>
    {/if}
    <label class="check"><input type="checkbox" bind:checked={s.wrap} /> Wrap</label>
  </div>

  <div class="row buttons">
    <button class="btn" title="Find previous (Shift+Enter)" onclick={() => store.findNext(true, tab)}><Icon name="arrow-up" size={14} /></button>
    <button class="btn" title="Find next (Enter)" onclick={() => store.findNext(false, tab)}><Icon name="arrow-down" size={14} /></button>
    <span class="grow"></span>
    {#if running}
      <button class="btn" onclick={() => running?.cancel()}>Cancel {Math.round(progress * 100)}%</button>
    {:else}
      <button class="btn primary" onclick={findAll} title="Find all (Ctrl/⌘+Enter)">Find all</button>
    {/if}
  </div>

  <div class="field replace">
    <input
      class:mono={s.mode !== "text"}
      bind:value={s.replace}
      placeholder={s.mode === "hex" ? "Replace with hex bytes" : s.mode === "number" ? "Replace with number" : "Replace with"}
      onkeydown={(e) => e.stopPropagation()}
      disabled={tab.info.readonly}
      spellcheck="false"
    />
  </div>
  <div class="row buttons">
    <span class="grow hint">{s.mode === "regex" ? "Regex replacement is literal text" : ""}</span>
    <button class="btn" disabled={tab.info.readonly || !!running} onclick={replaceOne}>Replace</button>
    <button class="btn" disabled={tab.info.readonly || !!running} onclick={replaceAll}>Replace all</button>
  </div>
  {#if error}<div class="err">{error}</div>{/if}
</div>

<div class="results-head">
  {#if tab.matches.length}
    <span>{tab.matches.length.toLocaleString("en-US")}{truncated ? "+" : ""} matches</span>
    {#if stale}<span class="stale" title="The file changed since this search">outdated</span>{/if}
    <span class="grow"></span>
    <button class="btn ghost small" onclick={() => { tab.matches = []; tab.matchIndex = -1; }}>Clear</button>
  {:else}
    <span class="muted">Enter to find next, ⇧Enter for previous, {navigator.platform.includes("Mac") ? "⌘" : "Ctrl+"}Enter for all</span>
  {/if}
</div>
<VirtualList items={tab.matches} rowHeight={24}>
  {#snippet row(m, i)}
    <button
      class="result mono"
      class:current={i === tab.matchIndex}
      onclick={() => { tab.matchIndex = i; store.select(tab, m.offset, m.len); }}
    >
      <span class="idx">{i + 1}</span>
      <span>0x{hex(m.offset, 8)}</span>
      <span class="muted len">{m.len.toLocaleString("en-US")} B</span>
      {#if tab.regionAt(m.offset)}<span class="reg">{tab.regionAt(m.offset)?.name}</span>{/if}
    </button>
  {/snippet}
</VirtualList>

<style>
  .modes {
    width: 100%;
    display: flex;
  }
  .modes button {
    flex: 1;
  }
  .field {
    margin-top: 8px;
    display: flex;
  }
  .field input {
    flex: 1;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 12px;
    align-items: center;
    margin-top: 8px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
  }
  .buttons {
    margin-top: 8px;
  }
  .replace {
    margin-top: 14px;
  }
  .hint {
    color: var(--muted);
    font-size: 11.5px;
  }
  .err {
    margin-top: 8px;
    color: var(--danger);
    font-size: 12px;
    user-select: text;
    -webkit-user-select: text;
  }
  .results-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }
  .stale {
    color: var(--warning);
    font-size: 11px;
  }
  .result {
    width: 100%;
    height: 24px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border: none;
    background: transparent;
    cursor: pointer;
    font-size: 12px;
    text-align: left;
  }
  .result:hover {
    background: var(--hover);
  }
  .result.current {
    background: var(--accent-soft);
  }
  .len {
    white-space: nowrap;
    flex: none;
  }
  .idx {
    color: var(--muted);
    min-width: 34px;
  }
  .reg {
    color: var(--accent-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
