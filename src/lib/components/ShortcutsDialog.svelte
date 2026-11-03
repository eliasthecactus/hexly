<script lang="ts">
  import { commands } from "../commands.svelte";
  import { isMac, shortcutLabel } from "../format";
  import Modal from "./Modal.svelte";

  const groups = [...new Set(commands.map((c) => c.group))];
  const mod = isMac ? "⌘" : "Ctrl+";
  const editing: [string, string][] = [
    ["0–9, A–F", "Type a nibble in the hex column"],
    ["Any key", "Type a character in the text column"],
    ["Tab", "Switch between hex and text column"],
    ["Arrows / Page Up / Page Down", "Move the cursor (Shift extends the selection)"],
    [isMac ? "⌘↑ / ⌘↓" : "Ctrl+Home / Ctrl+End", "Start / end of file"],
    ["Home / End", "Start / end of row"],
    ["Backspace / Delete", "Delete bytes (insert mode only)"],
    [`${mod}C / ${mod}V / ${mod}X`, "Copy, paste, cut (hex in the hex column, text in the text column)"],
    ["Escape", "Clear the selection"],
  ];
</script>

<Modal title="Keyboard shortcuts" width={640} top>
  <div class="cols">
    <section>
      <h4>Editing</h4>
      {#each editing as [k, label]}
        <div class="line"><span>{label}</span><span class="kbd">{k}</span></div>
      {/each}
    </section>
    {#each groups as g}
      <section>
        <h4>{g}</h4>
        {#each commands.filter((c) => c.group === g && c.keys) as c}
          <div class="line"><span>{c.label}</span><span class="kbd">{shortcutLabel(c.keys![0])}</span></div>
        {/each}
      </section>
    {/each}
  </div>
</Modal>

<style>
  .cols {
    columns: 2;
    column-gap: 24px;
  }
  section {
    break-inside: avoid;
    margin-bottom: 14px;
  }
  h4 {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 3px 0;
    font-size: 12.5px;
  }
  .line span:first-child {
    color: var(--text-2);
  }
  .kbd {
    flex: none;
  }
</style>
