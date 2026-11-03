<script lang="ts">
  import Self from "./TreeItem.svelte";
  import Icon from "../components/Icon.svelte";
  import { store, type Tab } from "../store.svelte";
  import type { TreeNode } from "../types";

  let { node, tab, depth = 0, open: initiallyOpen = false }: { node: TreeNode; tab: Tab; depth?: number; open?: boolean } = $props();
  // svelte-ignore state_referenced_locally
  let open = $state(initiallyOpen);
  const kids = $derived(node.children ?? []);
  const hasRange = $derived(node.offset !== undefined && node.size !== undefined);
  const LIMIT = 500;
  let showAll = $state(false);
</script>

<div
  class="item"
  role="treeitem"
  aria-selected="false"
  aria-expanded={kids.length ? open : undefined}
  tabindex="-1"
  style:padding-left="{8 + depth * 14}px"
  class:clickable={hasRange || kids.length}
  onmouseenter={() => hasRange && (tab.highlight = { offset: node.offset!, len: Math.max(1, node.size!) })}
  onmouseleave={() => (tab.highlight = null)}
  onclick={() => {
    if (hasRange) store.select(tab, node.offset!, Math.max(1, node.size!));
    if (kids.length) open = !open;
  }}
  onkeydown={() => {}}
>
  <span class="chev">
    {#if kids.length}<Icon name={open ? "chevron-down" : "chevron-right"} size={13} />{/if}
  </span>
  <span class="label">{node.label}</span>
  {#if node.value}<span class="value mono">{node.value}</span>{/if}
</div>
{#if open}
  {#each showAll ? kids : kids.slice(0, LIMIT) as child}
    <Self node={child} {tab} depth={depth + 1} />
  {/each}
  {#if !showAll && kids.length > LIMIT}
    <button class="more" style:margin-left="{30 + depth * 14}px" onclick={() => (showAll = true)}>
      Show all {kids.length.toLocaleString("en-US")}
    </button>
  {/if}
{/if}

<style>
  .item {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 24px;
    padding-right: 10px;
    font-size: 12.5px;
  }
  .item.clickable {
    cursor: pointer;
  }
  .item:hover {
    background: var(--hover);
  }
  .chev {
    width: 14px;
    flex: none;
    display: inline-flex;
    color: var(--muted);
  }
  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: none;
    max-width: 55%;
  }
  .value {
    color: var(--text-2);
    font-size: 11.5px;
    margin-left: auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-left: 10px;
    user-select: text;
    -webkit-user-select: text;
  }
  .more {
    border: none;
    background: none;
    color: var(--accent-2);
    cursor: pointer;
    padding: 4px 0;
    font-size: 12px;
  }
</style>
