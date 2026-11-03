<script lang="ts">
  import { api } from "../api";
  import { store, type Tab } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { hex } from "../format";
  import type { Field, NumType } from "../types";

  let { tab }: { tab: Tab } = $props();

  let fields = $state<Field[]>([]);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let error = $state("");

  const offset = $derived(tab.selection.len > 0 ? tab.selection.start : tab.cursor);

  $effect(() => {
    const off = offset;
    const le = settings.data.littleEndian;
    void tab.info.stateId;
    const id = tab.id;
    let cancelled = false;
    const timer = setTimeout(() => {
      api.inspect(id, off, le).then((f) => {
        if (!cancelled) fields = f;
      }).catch(() => {});
    }, 16);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  const va = $derived(tab.vaOf(offset));
  const region = $derived(tab.regionAt(offset));

  function startEdit(f: Field) {
    if (!f.editable || tab.info.readonly) return;
    editing = f.key;
    draft = f.value === "—" ? "" : f.value;
    error = "";
  }

  async function commit(f: Field) {
    try {
      const info = await api.writeValue(tab.id, offset, f.key as NumType, draft, settings.data.littleEndian);
      store.applyInfo(tab, info);
      editing = null;
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="panel-section">
  <div class="pos">
    <div><span class="muted">Offset</span><span class="mono">0x{hex(offset)}</span></div>
    <div><span class="muted">Decimal</span><span class="mono">{offset.toLocaleString("en-US")}</span></div>
    {#if va !== null}<div><span class="muted">Virtual address</span><span class="mono">0x{hex(va)}</span></div>{/if}
    {#if region}<div><span class="muted">In</span><span class="region">{region.name}</span></div>{/if}
  </div>
</div>

<div class="panel-section head">
  <p class="panel-title">Values at cursor</p>
  <div class="segmented">
    <button class:on={settings.data.littleEndian} onclick={() => settings.set("littleEndian", true)}>Little endian</button>
    <button class:on={!settings.data.littleEndian} onclick={() => settings.set("littleEndian", false)}>Big endian</button>
  </div>
</div>

<div class="list">
  {#each fields as f (f.key)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="field"
      class:editable={f.editable && !tab.info.readonly}
      onmouseenter={() => (tab.highlight = { offset, len: f.size })}
      onmouseleave={() => (tab.highlight = null)}
      ondblclick={() => startEdit(f)}
      title={f.editable && !tab.info.readonly ? "Double-click to edit" : ""}
    >
      <span class="label">{f.label}</span>
      {#if editing === f.key}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="mono"
          autofocus
          bind:value={draft}
          onkeydown={(e) => {
            if (e.key === "Enter") commit(f);
            if (e.key === "Escape") editing = null;
            e.stopPropagation();
          }}
          onblur={() => (editing = null)}
        />
      {:else}
        <span class="value mono" class:none={f.value === "—"}>{f.value}</span>
      {/if}
    </div>
    {#if editing === f.key && error}<div class="err">{error}</div>{/if}
  {/each}
</div>

<style>
  .pos {
    display: grid;
    gap: 5px;
  }
  .pos div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .region {
    color: var(--accent-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .head .panel-title {
    margin: 0;
  }
  .list {
    padding: 4px 0 12px;
  }
  .field {
    display: grid;
    grid-template-columns: 104px 1fr;
    align-items: center;
    gap: 8px;
    padding: 3px 12px;
    min-height: 26px;
  }
  .field:hover {
    background: var(--hover);
  }
  .field.editable {
    cursor: text;
  }
  .label {
    color: var(--muted);
    font-size: 12px;
  }
  .value {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
    -webkit-user-select: text;
  }
  .none {
    color: var(--muted);
  }
  .field input {
    padding: 2px 6px;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    padding: 0 12px 6px 124px;
  }
</style>
