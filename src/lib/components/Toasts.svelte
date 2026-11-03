<script lang="ts">
  import { store } from "../store.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts">
  {#if store.update}
    <div class="toast update">
      <Icon name="download" size={16} />
      <span class="grow">Hexly {store.update.version} is available.</span>
      <button class="btn small primary" onclick={() => store.update?.install().catch((e) => store.error(e))}>Install &amp; restart</button>
      <button class="icon-btn" title="Later" onclick={() => (store.update = null)}><Icon name="x" size={13} /></button>
    </div>
  {/if}
  {#each store.toasts as t (t.id)}
    <div class="toast {t.kind}">
      <Icon name={t.kind === "error" ? "alert" : t.kind === "success" ? "check" : "info"} size={15} />
      <span class="grow">{t.text}</span>
      {#if t.action}<button class="btn small" onclick={() => { t.action?.run(); store.dismiss(t.id); }}>{t.action.label}</button>{/if}
      <button class="icon-btn" title="Dismiss" onclick={() => store.dismiss(t.id)}><Icon name="x" size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 70;
    width: min(520px, calc(100vw - 32px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 12px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: var(--shadow);
    font-size: 12.5px;
    line-height: 1.4;
    animation: up 0.16s ease-out;
    user-select: text;
    -webkit-user-select: text;
  }
  .toast .icon-btn {
    width: 22px;
    height: 22px;
  }
  .error {
    border-color: color-mix(in srgb, var(--danger) 55%, var(--border-strong));
  }
  .error > :global(svg:first-child) {
    color: var(--danger);
  }
  .success > :global(svg:first-child) {
    color: var(--success);
  }
  .info > :global(svg:first-child),
  .update > :global(svg:first-child) {
    color: var(--accent-2);
  }
  .grow {
    flex: 1;
  }
  @keyframes up {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
</style>
