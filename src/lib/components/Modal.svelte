<script lang="ts">
  import type { Snippet } from "svelte";
  import { store } from "../store.svelte";

  let { title, width = 440, children, footer, top = false }: {
    title?: string;
    width?: number;
    children: Snippet;
    footer?: Snippet;
    top?: boolean;
  } = $props();

  function close() {
    store.dialog = null;
    queueMicrotask(() => (document.querySelector("canvas[data-hexview='main']") as HTMLElement | null)?.focus());
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="backdrop"
  class:top
  onmousedown={(e) => e.target === e.currentTarget && close()}
  onkeydown={(e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }}
>
  <div class="modal" style:width="{width}px" role="dialog" aria-modal="true" aria-label={title}>
    {#if title}<div class="head">{title}</div>{/if}
    <div class="body">{@render children()}</div>
    {#if footer}<div class="foot">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(5, 8, 15, 0.45);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 14vh;
    z-index: 50;
    animation: fade 0.12s ease-out;
  }
  .backdrop.top {
    padding-top: 9vh;
  }
  .modal {
    max-width: calc(100vw - 32px);
    max-height: 76vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: var(--shadow);
    overflow: hidden;
    animation: pop 0.14s ease-out;
  }
  .head {
    padding: 14px 16px 0;
    font-weight: 600;
    font-size: 14px;
  }
  .body {
    padding: 14px 16px;
    overflow: auto;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      transform: translateY(-6px) scale(0.99);
      opacity: 0;
    }
  }
</style>
