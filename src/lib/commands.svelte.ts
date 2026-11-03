import { openUrl } from "@tauri-apps/plugin-opener";
import { exit } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";
import { isMac, matchShortcut } from "./format";
import { settings } from "./settings.svelte";
import { store } from "./store.svelte";
import { checkForUpdates } from "./updater";

export interface Command {
  id: string;
  label: string;
  group: string;
  /** Shortcuts like `Mod+Shift+F`; the first one is shown in menus. */
  keys?: string[];
  /** Handled by the native macOS menu (skip the JS key handler there). */
  menu?: boolean;
  /** Needs an open document. */
  needsDoc?: boolean;
  run: () => unknown;
}

function inInput(): HTMLInputElement | HTMLTextAreaElement | null {
  const el = document.activeElement;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return el;
  return null;
}

const t = () => store.active;

export function setPanel(panel: string, toggle = true) {
  if (toggle && settings.data.sidebarOpen && settings.data.sidebarPanel === panel) {
    settings.set("sidebarOpen", false);
  } else {
    settings.set("sidebarPanel", panel);
    settings.set("sidebarOpen", true);
  }
}

function zoom(delta: number) {
  const next = delta === 0 ? 13 : Math.max(9, Math.min(24, settings.data.fontSize + delta));
  settings.set("fontSize", next);
}

export async function quit() {
  const dirty = store.tabs.filter((x) => x.info.modified);
  if (dirty.length) {
    const ok = await ask(
      `${dirty.length === 1 ? `"${dirty[0].info.name}" has` : `${dirty.length} files have`} unsaved changes. Quit anyway?`,
      { title: "Unsaved changes", kind: "warning", okLabel: "Quit", cancelLabel: "Cancel" },
    );
    if (!ok) return;
  }
  await exit(0);
}

export const commands: Command[] = [
  // file
  { id: "new", label: "New File", group: "File", keys: ["Mod+N"], menu: true, run: () => store.newFile() },
  { id: "open", label: "Open…", group: "File", keys: ["Mod+O"], menu: true, run: () => store.openWithDialog() },
  { id: "open-readonly", label: "Open Read-Only…", group: "File", keys: ["Mod+Shift+O"], menu: true, run: () => store.openWithDialog(true) },
  { id: "save", label: "Save", group: "File", keys: ["Mod+S"], menu: true, needsDoc: true, run: () => store.save() },
  { id: "save-as", label: "Save As…", group: "File", keys: ["Mod+Shift+S"], menu: true, needsDoc: true, run: () => store.saveAs() },
  { id: "export-selection", label: "Export Selection…", group: "File", keys: ["Mod+Alt+E"], menu: true, needsDoc: true, run: () => store.exportSelection() },
  { id: "reload", label: "Revert to Saved", group: "File", menu: true, needsDoc: true, run: () => store.revert() },
  { id: "compare", label: "Compare With…", group: "File", keys: ["Mod+Alt+D"], menu: true, needsDoc: true, run: () => store.compareWith() },
  { id: "close-compare", label: "Close Comparison", group: "File", needsDoc: true, run: () => store.closeCompare() },
  { id: "close-tab", label: "Close Tab", group: "File", keys: ["Mod+W"], menu: true, needsDoc: true, run: () => t() && store.close(t()!) },
  { id: "quit", label: "Quit Hexly", group: "File", menu: true, run: quit },

  // edit
  {
    id: "undo", label: "Undo", group: "Edit", keys: ["Mod+Z"], menu: true,
    run: () => (inInput() ? document.execCommand("undo") : store.undo()),
  },
  {
    id: "redo", label: "Redo", group: "Edit", keys: ["Mod+Shift+Z", "Mod+Y"], menu: true,
    run: () => (inInput() ? document.execCommand("redo") : store.redo()),
  },
  {
    id: "select-all", label: "Select All", group: "Edit", keys: ["Mod+A"], menu: true,
    run: () => {
      const el = inInput();
      if (el) el.select();
      else store.selectAll();
    },
  },
  { id: "copy-hex", label: "Copy as Hex", group: "Edit", needsDoc: true, run: () => store.copy(t(), "hex") },
  { id: "copy-text", label: "Copy as Text", group: "Edit", needsDoc: true, run: () => store.copy(t(), "text") },
  { id: "copy-as", label: "Copy As…", group: "Edit", keys: ["Mod+Shift+C"], menu: true, needsDoc: true, run: () => (store.dialog = "copyAs") },
  { id: "toggle-insert", label: "Toggle Insert / Overwrite", group: "Edit", keys: ["Mod+I", "Insert"], menu: true, needsDoc: true, run: () => store.toggleInsert() },
  { id: "fill", label: "Fill Selection…", group: "Edit", keys: ["Mod+Shift+F"], menu: true, needsDoc: true, run: () => (store.dialog = "fill") },
  { id: "insert-bytes", label: "Insert Bytes…", group: "Edit", keys: ["Mod+Shift+I"], menu: true, needsDoc: true, run: () => (store.dialog = "insert") },
  { id: "find", label: "Find…", group: "Edit", keys: ["Mod+F"], menu: true, run: () => store.openSearch() },
  { id: "find-next", label: "Find Next", group: "Edit", keys: ["Mod+G", "F3"], menu: true, needsDoc: true, run: () => store.findNext(false) },
  { id: "find-prev", label: "Find Previous", group: "Edit", keys: ["Mod+Shift+G", "Shift+F3"], menu: true, needsDoc: true, run: () => store.findNext(true) },

  // view
  { id: "palette", label: "Command Palette…", group: "View", keys: ["Mod+K", "Mod+Shift+P"], menu: true, run: () => (store.dialog = store.dialog === "palette" ? null : "palette") },
  { id: "toggle-sidebar", label: "Toggle Sidebar", group: "View", keys: ["Mod+B"], menu: true, run: () => settings.set("sidebarOpen", !settings.data.sidebarOpen) },
  { id: "zoom-in", label: "Zoom In", group: "View", keys: ["Mod+="], menu: true, run: () => zoom(1) },
  { id: "zoom-out", label: "Zoom Out", group: "View", keys: ["Mod+-"], menu: true, run: () => zoom(-1) },
  { id: "zoom-reset", label: "Actual Size", group: "View", keys: ["Mod+0"], menu: true, run: () => zoom(0) },
  { id: "panel-inspector", label: "Show Data Inspector", group: "View", keys: ["Mod+1"], run: () => setPanel("inspector") },
  { id: "panel-structure", label: "Show Structure", group: "View", keys: ["Mod+2"], run: () => setPanel("structure") },
  { id: "panel-search", label: "Show Search", group: "View", keys: ["Mod+3"], run: () => setPanel("search") },
  { id: "panel-strings", label: "Show Strings", group: "View", keys: ["Mod+4"], run: () => setPanel("strings") },
  { id: "panel-disasm", label: "Show Disassembly", group: "View", keys: ["Mod+5"], run: () => setPanel("disasm") },
  { id: "panel-bookmarks", label: "Show Bookmarks", group: "View", keys: ["Mod+6"], run: () => setPanel("bookmarks") },
  { id: "panel-hashes", label: "Show Hashes", group: "View", keys: ["Mod+7"], run: () => setPanel("hashes") },
  { id: "toggle-endian", label: "Toggle Little / Big Endian", group: "View", run: () => settings.set("littleEndian", !settings.data.littleEndian) },
  {
    id: "cycle-text", label: "Cycle Text Encoding (ASCII / Latin-1 / EBCDIC)", group: "View",
    run: () => {
      const order = ["ascii", "latin1", "ebcdic"] as const;
      settings.set("textMode", order[(order.indexOf(settings.data.textMode) + 1) % order.length]);
    },
  },
  {
    id: "toggle-theme", label: "Toggle Light / Dark Theme", group: "View",
    run: () => settings.set("theme", document.documentElement.dataset.theme === "dark" ? "light" : "dark"),
  },
  { id: "settings", label: "Settings…", group: "View", keys: ["Mod+,"], menu: true, run: () => (store.dialog = "settings") },

  // go
  { id: "goto", label: "Go to Offset…", group: "Go", keys: ["Mod+L"], menu: true, needsDoc: true, run: () => (store.dialog = "goto") },
  { id: "goto-entry", label: "Go to Entry Point", group: "Go", keys: ["Mod+E"], menu: true, needsDoc: true, run: () => store.gotoEntry() },
  { id: "follow-pointer", label: "Follow Pointer (64-bit)", group: "Go", keys: ["Mod+J"], needsDoc: true, run: () => store.followPointer(t(), 8) },
  { id: "follow-pointer-32", label: "Follow Pointer (32-bit)", group: "Go", keys: ["Mod+Shift+J"], needsDoc: true, run: () => store.followPointer(t(), 4) },
  { id: "next-diff", label: "Next Difference", group: "Go", keys: ["Mod+Alt+Down"], menu: true, needsDoc: true, run: () => store.jumpDifference(1) },
  { id: "prev-diff", label: "Previous Difference", group: "Go", keys: ["Mod+Alt+Up"], menu: true, needsDoc: true, run: () => store.jumpDifference(-1) },
  { id: "toggle-bookmark", label: "Toggle Bookmark", group: "Go", keys: ["Mod+D"], menu: true, needsDoc: true, run: () => store.toggleBookmark() },
  { id: "next-bookmark", label: "Next Bookmark", group: "Go", keys: ["F2"], menu: true, needsDoc: true, run: () => store.jumpBookmark(1) },
  { id: "prev-bookmark", label: "Previous Bookmark", group: "Go", keys: ["Shift+F2"], menu: true, needsDoc: true, run: () => store.jumpBookmark(-1) },
  { id: "next-tab", label: "Next Tab", group: "Go", keys: ["Ctrl+Tab"], menu: true, run: () => store.cycleTab(1) },
  { id: "prev-tab", label: "Previous Tab", group: "Go", keys: ["Ctrl+Shift+Tab"], menu: true, run: () => store.cycleTab(-1) },

  // help
  { id: "shortcuts", label: "Keyboard Shortcuts", group: "Help", keys: ["Mod+/"], menu: true, run: () => (store.dialog = "shortcuts") },
  { id: "check-updates", label: "Check for Updates…", group: "Help", menu: true, run: () => checkForUpdates(true) },
  { id: "website", label: "Hexly on GitHub", group: "Help", menu: true, run: () => openUrl("https://github.com/eliasthecactus/hexly") },
];

const byId = new Map(commands.map((c) => [c.id, c]));

export function runCommand(id: string) {
  const c = byId.get(id);
  if (!c) return;
  if (c.needsDoc && !store.active) return;
  Promise.resolve(c.run()).catch((e) => store.error(e));
}

/** Global keyboard handler. Returns true if the event was handled. */
export function handleShortcut(e: KeyboardEvent): boolean {
  const typing = !!inInput();
  for (const c of commands) {
    if (!c.keys) continue;
    if (isMac && c.menu) continue; // the native menu delivers these
    for (const k of c.keys) {
      if (!matchShortcut(e, k)) continue;
      // inside text fields, leave plain editing keys to the field
      if (typing && ["undo", "redo", "select-all", "toggle-insert"].includes(c.id)) return false;
      if (typing && !k.includes("Mod") && !k.startsWith("F")) return false;
      e.preventDefault();
      runCommand(c.id);
      return true;
    }
  }
  return false;
}

export function shortcutFor(id: string): string | undefined {
  return byId.get(id)?.keys?.[0];
}
