<script lang="ts">
  import { api, errorText } from "../api";
  import { store, type Tab } from "../store.svelte";
  import { hex } from "../format";
  import type { Arch, Insn } from "../types";

  let { tab }: { tab: Tab } = $props();

  const ARCH_LABELS: Record<Arch, string> = {
    x86: "x86 (32-bit)", x86_64: "x86-64", arm: "ARM", thumb: "ARM Thumb", arm64: "ARM64 / AArch64",
    riscv32: "RISC-V 32", riscv64: "RISC-V 64", mips32: "MIPS32 BE", mips32le: "MIPS32 LE",
    mips64: "MIPS64 BE", mips64le: "MIPS64 LE", ppc32: "PowerPC 32", ppc64: "PowerPC 64 BE", ppc64le: "PowerPC 64 LE",
  };

  let choice = $state<Arch | "auto">("auto");
  let insns = $state.raw<Insn[]>([]);
  let usedArch = $state<Arch | null>(null);
  let error = $state("");
  let follow = $state(true);
  let showBytes = $state(false);
  let anchor = $state(0);

  const start = $derived(tab.selection.len > 0 ? tab.selection.start : tab.cursor);

  $effect(() => {
    if (follow) anchor = start;
  });

  $effect(() => {
    const off = anchor;
    const arch = choice === "auto" ? null : choice;
    void tab.info.stateId;
    void tab.format;
    const id = tab.id;
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const r = await api.disassemble(id, off, 160, arch);
        if (cancelled) return;
        insns = r.insns;
        usedArch = r.arch;
        error = "";
      } catch (e) {
        if (!cancelled) error = errorText(e);
      }
    }, 40);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  async function jump(target: number) {
    const off = tab.format?.mappings.length ? await api.vaToOffset(tab.id, target) : target;
    if (off === null || off >= tab.info.size) return store.toast(`0x${hex(target)} is not in the file`);
    store.select(tab, off, 1);
  }

  const symbolAt = $derived.by(() => {
    const map = new Map<number, string>();
    for (const s of tab.format?.symbols ?? []) if (s.kind === "function" || s.kind === "symbol" || s.kind === "export") map.set(s.vaddr, s.name);
    return map;
  });
</script>

<div class="panel-section row">
  <select bind:value={choice} class="grow">
    <option value="auto">Auto{tab.format?.arch ? ` (${ARCH_LABELS[tab.format.arch]})` : " (pick one)"}</option>
    {#each Object.entries(ARCH_LABELS) as [k, label]}<option value={k}>{label}</option>{/each}
  </select>
  <label class="check" title="Show instruction bytes"><input type="checkbox" bind:checked={showBytes} /> Bytes</label>
  <label class="check" title="Disassemble from the cursor as it moves"><input type="checkbox" bind:checked={follow} /> Follow</label>
</div>

{#if error}
  <div class="empty err">{error}</div>
{:else if !usedArch}
  <div class="empty">No architecture detected for this file. Pick one above to disassemble from the cursor.</div>
{:else}
  <div class="list mono" class:bytes-on={showBytes}>
    {#each insns as ins (ins.offset)}
      {#if symbolAt.has(ins.address)}
        <div class="sym">{symbolAt.get(ins.address)}:</div>
      {/if}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ins"
        class:cur={ins.offset === tab.cursor}
        class:bad={ins.mnemonic === ".byte"}
        onclick={() => store.select(tab, ins.offset, ins.size)}
        onmouseenter={() => (tab.highlight = { offset: ins.offset, len: ins.size })}
        onmouseleave={() => (tab.highlight = null)}
      >
        <span class="addr">{hex(ins.address, 8)}</span>
        {#if showBytes}<span class="bytes">{ins.bytes}</span>{/if}
        <span class="mn">{ins.mnemonic}</span>
        {#if ins.target !== undefined}
          <button class="op target" onclick={(e) => { e.stopPropagation(); jump(ins.target!); }} title="Go to target">{ins.operands}</button>
        {:else}
          <span class="op">{ins.operands}</span>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
    white-space: nowrap;
  }
  .list {
    flex: 1;
    overflow: auto;
    font-size: 12px;
    padding: 4px 0 16px;
  }
  .sym {
    padding: 8px 12px 2px;
    color: var(--warning);
  }
  .ins {
    display: grid;
    grid-template-columns: 76px 58px 1fr;
    gap: 6px;
    padding: 1px 12px;
    cursor: pointer;
    line-height: 20px;
    white-space: nowrap;
  }
  .bytes-on .ins {
    grid-template-columns: 76px 100px 58px 1fr;
  }
  .ins:hover {
    background: var(--hover);
  }
  .ins.cur {
    background: var(--accent-soft);
  }
  .addr {
    color: var(--muted);
  }
  .bytes {
    color: var(--hx-zero);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mn {
    color: var(--hx-printable);
  }
  .bad .mn {
    color: var(--muted);
  }
  .op {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
    user-select: text;
    -webkit-user-select: text;
  }
  .target {
    border: none;
    background: none;
    padding: 0;
    text-align: left;
    font: inherit;
    color: var(--accent-2);
    text-decoration: underline dotted;
    cursor: pointer;
  }
  .err {
    color: var(--danger);
  }
</style>
