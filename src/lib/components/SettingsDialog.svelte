<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { store } from "../store.svelte";
  import { settings, type BytesPerRow, type Theme } from "../settings.svelte";
  import { checkForUpdates } from "../updater";
  import Modal from "./Modal.svelte";

  let version = $state("");
  getVersion().then((v) => (version = v)).catch(() => {});
  const d = settings.data;
  const set = settings.set.bind(settings);
</script>

<Modal title="Settings" width={520} top>
  <div class="grid">
    <span>Theme</span>
    <div class="segmented">
      {#each [["system", "System"], ["dark", "Dark"], ["light", "Light"]] as [v, l]}
        <button class:on={d.theme === v} onclick={() => set("theme", v as Theme)}>{l}</button>
      {/each}
    </div>

    <span>Font size</span>
    <div class="row">
      <input type="range" min="9" max="24" value={d.fontSize} oninput={(e) => set("fontSize", +e.currentTarget.value)} />
      <span class="mono">{d.fontSize}px</span>
    </div>

    <span>Bytes per row</span>
    <select value={String(d.bytesPerRow)} onchange={(e) => {
      const v = e.currentTarget.value;
      set("bytesPerRow", (v === "auto" ? "auto" : +v) as BytesPerRow);
    }}>
      <option value="auto">Fit to window</option>
      {#each [8, 16, 24, 32, 48, 64] as n}<option value={String(n)}>{n}</option>{/each}
    </select>

    <span>Offsets</span>
    <div class="segmented">
      <button class:on={d.offsetBase === "hex"} onclick={() => set("offsetBase", "hex")}>Hexadecimal</button>
      <button class:on={d.offsetBase === "dec"} onclick={() => set("offsetBase", "dec")}>Decimal</button>
    </div>

    <span>Text column</span>
    <select value={d.textMode} onchange={(e) => set("textMode", e.currentTarget.value as typeof d.textMode)}>
      <option value="ascii">ASCII</option>
      <option value="latin1">Latin-1 (ISO 8859-1)</option>
      <option value="ebcdic">EBCDIC (code page 037)</option>
    </select>

    <span>Display</span>
    <div class="checks">
      <label><input type="checkbox" checked={d.uppercase} onchange={(e) => set("uppercase", e.currentTarget.checked)} /> Uppercase hex</label>
      <label><input type="checkbox" checked={d.colorBytes} onchange={(e) => set("colorBytes", e.currentTarget.checked)} /> Color bytes by type (00, text, control, high, FF)</label>
      <label><input type="checkbox" checked={d.showSections} onchange={(e) => set("showSections", e.currentTarget.checked)} /> Show section stripe next to offsets</label>
    </div>

    <span>Updates</span>
    <div class="checks">
      <label><input type="checkbox" checked={d.checkUpdates} onchange={(e) => set("checkUpdates", e.currentTarget.checked)} /> Check for updates on start</label>
      <div><button class="btn small" onclick={() => checkForUpdates(true)}>Check now</button></div>
    </div>
  </div>
  {#snippet footer()}
    <span class="ver muted">Hexly {version}</span>
    <button class="btn" onclick={() => settings.reset()}>Reset to defaults</button>
    <button class="btn primary" onclick={() => (store.dialog = null)}>Done</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 14px 16px;
    align-items: center;
  }
  .grid > span {
    color: var(--text-2);
  }
  .checks {
    display: grid;
    gap: 8px;
  }
  .checks label {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  input[type="range"] {
    flex: 1;
    accent-color: var(--accent);
    padding: 0;
    border: none;
    box-shadow: none;
  }
  .ver {
    margin-right: auto;
    align-self: center;
    font-size: 12px;
  }
</style>
