<script lang="ts">
  import { store } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { basename, isMac, shortcutLabel } from "../format";
  import { shortcutFor } from "../commands.svelte";
  import Icon from "./Icon.svelte";
  import logo from "../../../assets/icon.svg";

  const k = (id: string) => shortcutLabel(shortcutFor(id) ?? "");
  const dir = (p: string) => p.slice(0, Math.max(0, p.length - basename(p).length - 1));
</script>

<div class="welcome">
  <div class="card">
    <img src={logo} alt="" class="logo" />
    <h1>Hexly</h1>
    <p class="tag">A fast hex editor for binaries.</p>

    <div class="actions">
      <button class="btn primary big" onclick={() => store.openWithDialog()}>
        <Icon name="folder" size={16} /> Open file <span class="kbd">{k("open")}</span>
      </button>
      <button class="btn big" onclick={() => store.newFile()}>
        <Icon name="plus" size={16} /> New file <span class="kbd">{k("new")}</span>
      </button>
    </div>
    <p class="drop">or drop files anywhere in this window</p>

    {#if settings.data.recent.length}
      <div class="recent">
        <div class="panel-title">Recent</div>
        {#each settings.data.recent.slice(0, 8) as p}
          <div class="ritem">
            <button class="rbtn" onclick={() => store.open(p)} title={p}>
              <Icon name="file" size={14} />
              <span class="rname">{basename(p)}</span>
              <span class="rdir">{dir(p)}</span>
            </button>
            <button class="icon-btn x" title="Remove from list" onclick={() => settings.removeRecent(p)}><Icon name="x" size={12} /></button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="tips">
      <span><span class="kbd">{k("palette")}</span> commands</span>
      <span><span class="kbd">{k("goto")}</span> go to offset</span>
      <span><span class="kbd">{k("find")}</span> search</span>
      <span><span class="kbd">{isMac ? "⌘/" : "Ctrl+/"}</span> all shortcuts</span>
    </div>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    display: grid;
    place-items: center;
    overflow: auto;
    background:
      radial-gradient(1000px 500px at 50% -10%, var(--accent-soft), transparent 60%),
      var(--hx-bg);
  }
  .card {
    width: min(520px, calc(100% - 32px));
    text-align: center;
    padding: 32px 0;
  }
  .logo {
    width: 96px;
    height: 96px;
  }
  h1 {
    margin: 6px 0 2px;
    font-size: 28px;
    letter-spacing: -0.02em;
  }
  .tag {
    margin: 0;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    gap: 10px;
    justify-content: center;
    margin-top: 26px;
  }
  .big {
    height: 38px;
    padding: 0 16px;
    font-size: 13.5px;
  }
  .big .kbd {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.18);
    color: inherit;
  }
  .drop {
    color: var(--muted);
    font-size: 12px;
    margin: 10px 0 0;
  }
  .recent {
    margin-top: 28px;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 8px 8px;
    background: var(--panel);
  }
  .recent .panel-title {
    padding: 0 8px;
  }
  .ritem {
    display: flex;
    align-items: center;
  }
  .rbtn {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    background: transparent;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    color: var(--text-2);
  }
  .rbtn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .rname {
    color: var(--text);
    white-space: nowrap;
  }
  .rdir {
    color: var(--muted);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .x {
    width: 24px;
    height: 24px;
    opacity: 0;
  }
  .ritem:hover .x {
    opacity: 1;
  }
  .tips {
    margin-top: 26px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px 18px;
    justify-content: center;
    color: var(--muted);
    font-size: 12px;
  }
</style>
