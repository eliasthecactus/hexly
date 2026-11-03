import { ask, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errorText } from "./api";
import { ByteCache } from "./cache";
import { basename, fmtCount, hex } from "./format";
import { settings } from "./settings.svelte";
import type { Bookmark, DocInfo, EntropyMap, FormatInfo, Match, Query } from "./types";

export type Pane = "hex" | "text";

export interface CompareState {
  info: DocInfo;
  cache: ByteCache;
  diffCount: number | null;
}

export class Tab {
  info = $state<DocInfo>() as DocInfo;
  cache: ByteCache;
  cursor = $state(0);
  nibble = $state(0);
  pane = $state<Pane>("hex");
  anchor = $state<number | null>(null);
  topRow = $state(0);
  insertMode = $state(false);
  format = $state.raw<FormatInfo | null>(null);
  entropy = $state.raw<EntropyMap | null>(null);
  bookmarks = $state<Bookmark[]>([]);
  matches = $state.raw<Match[]>([]);
  matchIndex = $state(-1);
  highlight = $state<{ offset: number; len: number } | null>(null);
  compare = $state.raw<CompareState | null>(null);
  /** Bumped whenever displayed bytes may have changed. */
  version = $state(0);
  ignoreDiskChange = false;

  constructor(info: DocInfo) {
    this.info = info;
    this.cache = new ByteCache(info.id, () => this.version++);
    this.cache.size = info.size;
  }

  get id() {
    return this.info.id;
  }

  /** Selected range; `len` is 0 when nothing is selected (just a cursor). */
  get selection(): { start: number; end: number; len: number } {
    if (this.anchor === null) {
      return { start: this.cursor, end: this.cursor, len: 0 };
    }
    const start = Math.min(this.anchor, this.cursor);
    const end = Math.min(Math.max(this.anchor, this.cursor) + 1, this.info.size);
    return { start, end, len: Math.max(0, end - start) };
  }

  /** Selection, or the single byte under the cursor when nothing is selected. */
  get target(): { start: number; len: number } {
    const s = this.selection;
    if (s.len > 0) return { start: s.start, len: s.len };
    return { start: this.cursor, len: this.cursor < this.info.size ? 1 : 0 };
  }

  regionAt(offset: number) {
    const regs = this.format?.regions;
    if (!regs) return null;
    let best: (typeof regs)[number] | null = null;
    for (const r of regs) {
      if (offset >= r.offset && offset < r.offset + r.size) {
        if (!best || r.size <= best.size) best = r;
      }
    }
    return best;
  }

  vaOf(offset: number): number | null {
    const maps = this.format?.mappings;
    if (!maps) return null;
    for (const m of maps) {
      if (m.fsize > 0 && m.vsize > 0 && offset >= m.offset && offset < m.offset + m.fsize) return m.vaddr + (offset - m.offset);
    }
    return null;
  }
}

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error" | "success";
  action?: { label: string; run: () => void };
}

export type DialogKind = "goto" | "fill" | "insert" | "copyAs" | "settings" | "shortcuts" | "palette" | null;

export interface SearchState {
  mode: "hex" | "text" | "regex" | "number";
  hex: string;
  text: string;
  regex: string;
  number: string;
  encoding: "utf8" | "utf16le" | "utf16be" | "latin1";
  caseSensitive: boolean;
  numType: "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "f32" | "f64";
  replace: string;
  wrap: boolean;
}

const BOOKMARK_COLORS = ["#f59e0b", "#22c55e", "#3b82f6", "#ec4899", "#a855f7", "#14b8a6", "#ef4444"];

class Store {
  tabs = $state<Tab[]>([]);
  activeId = $state<number | null>(null);
  toasts = $state<Toast[]>([]);
  dialog = $state<DialogKind>(null);
  busy = $state<{ label: string; progress: number; cancel?: () => void } | null>(null);
  /** Visible layout of the active hex view (set by HexView). */
  view = $state({ rows: 32, bpr: 16 });
  focusSearch = $state(0);
  dragging = $state(false);
  update = $state<{ version: string; install: () => Promise<void> } | null>(null);
  search = $state<SearchState>({
    mode: "hex", hex: "", text: "", regex: "", number: "", encoding: "utf8", caseSensitive: false,
    numType: "u32", replace: "", wrap: true,
  });

  private toastId = 0;
  private analysisTimers = new Map<number, ReturnType<typeof setTimeout>>();
  private bookmarkTimers = new Map<number, ReturnType<typeof setTimeout>>();

  get active(): Tab | null {
    return this.tabs.find((t) => t.id === this.activeId) ?? null;
  }

  // ---- feedback -----------------------------------------------------------------

  toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"], ms = 4000) {
    const id = ++this.toastId;
    this.toasts.push({ id, text, kind, action });
    setTimeout(() => this.dismiss(id), kind === "error" ? Math.max(ms, 7000) : ms);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  error(e: unknown) {
    this.toast(errorText(e), "error");
  }

  updateTitle() {
    const t = this.active;
    const title = t ? `${t.info.name}${t.info.modified ? " •" : ""} — Hexly` : "Hexly";
    getCurrentWindow().setTitle(title).catch(() => {});
  }

  // ---- documents ----------------------------------------------------------------

  async open(path: string, readonly = false) {
    const existing = this.tabs.find((t) => t.info.path === path);
    if (existing) {
      this.activate(existing.id);
      return existing;
    }
    try {
      const info = await api.openFile(path, readonly);
      const again = this.tabs.find((t) => t.info.path === info.path);
      if (again) {
        await api.closeDocument(info.id);
        this.activate(again.id);
        return again;
      }
      const tab = new Tab(info);
      this.tabs.push(tab);
      this.activate(tab.id);
      settings.addRecent(info.path ?? path);
      this.afterOpen(this.tabs[this.tabs.length - 1]);
      if (info.readonly && !readonly) this.toast("Opened read-only: you don't have write access to this file.");
      return tab;
    } catch (e) {
      this.error(e);
      settings.removeRecent(path);
      return null;
    }
  }

  async openWithDialog(readonly = false) {
    const picked = await openDialog({ multiple: true, directory: false, title: readonly ? "Open read-only" : "Open file" });
    if (!picked) return;
    for (const p of Array.isArray(picked) ? picked : [picked]) await this.open(p, readonly);
  }

  async newFile() {
    const info = await api.newDocument();
    const tab = new Tab(info);
    tab.insertMode = true;
    this.tabs.push(tab);
    this.activate(tab.id);
  }

  private afterOpen(tab: Tab) {
    this.analyze(tab);
    api.loadBookmarks(tab.id).then((b) => { tab.bookmarks = Array.isArray(b) ? b : []; }).catch(() => {});
  }

  activate(id: number) {
    this.activeId = id;
    this.updateTitle();
  }

  async close(tab: Tab) {
    if (tab.info.modified) {
      const ok = await ask(`"${tab.info.name}" has unsaved changes. Close it anyway?`, {
        title: "Unsaved changes", kind: "warning", okLabel: "Discard changes", cancelLabel: "Cancel",
      });
      if (!ok) return;
    }
    const i = this.tabs.findIndex((t) => t.id === tab.id);
    if (i < 0) return;
    this.tabs.splice(i, 1);
    api.closeDocument(tab.id).catch(() => {});
    if (tab.compare) api.closeDocument(tab.compare.info.id).catch(() => {});
    if (this.activeId === tab.id) {
      const next = this.tabs[Math.min(i, this.tabs.length - 1)];
      this.activeId = next?.id ?? null;
    }
    this.updateTitle();
  }

  cycleTab(dir: 1 | -1) {
    if (this.tabs.length < 2) return;
    const i = this.tabs.findIndex((t) => t.id === this.activeId);
    const next = this.tabs[(i + dir + this.tabs.length) % this.tabs.length];
    this.activate(next.id);
  }

  async save(tab: Tab | null = this.active) {
    if (!tab) return;
    if (!tab.info.path || tab.info.readonly) return this.saveAs(tab);
    await this.withBusy("Saving…", async () => {
      try {
        const info = await api.save(tab.id);
        this.applyInfo(tab, info);
        this.toast(`Saved ${tab.info.name}`, "success", undefined, 2000);
      } catch (e) {
        this.error(e);
        api.docInfo(tab.id).then((i) => this.applyInfo(tab, i)).catch(() => {});
      }
    });
  }

  async saveAs(tab: Tab | null = this.active) {
    if (!tab) return;
    const path = await saveDialog({ defaultPath: tab.info.path ?? tab.info.name, title: "Save as" });
    if (!path) return;
    await this.withBusy("Saving…", async () => {
      try {
        const info = await api.saveAs(tab.id, path);
        this.applyInfo(tab, info);
        settings.addRecent(info.path ?? path);
        this.persistBookmarks(tab);
        this.toast(`Saved ${info.name}`, "success", undefined, 2000);
      } catch (e) {
        this.error(e);
      }
    });
  }

  async exportSelection(tab: Tab | null = this.active) {
    if (!tab) return;
    const { start, len } = tab.target;
    if (len === 0) return this.toast("Nothing selected");
    const path = await saveDialog({ defaultPath: `${tab.info.name}.${hex(start)}-${hex(start + len)}.bin`, title: "Export selection" });
    if (!path) return;
    try {
      await api.exportRange(tab.id, start, len, path);
      this.toast(`Exported ${fmtCount(len)} bytes to ${basename(path)}`, "success");
    } catch (e) {
      this.error(e);
    }
  }

  async revert(tab: Tab | null = this.active) {
    if (!tab?.info.path) return;
    if (tab.info.modified) {
      const ok = await ask("Discard all unsaved changes and reload the file from disk?", { kind: "warning", okLabel: "Revert" });
      if (!ok) return;
    }
    try {
      this.applyInfo(tab, await api.reload(tab.id));
      tab.ignoreDiskChange = false;
    } catch (e) {
      this.error(e);
    }
  }

  async checkDiskChange() {
    const tab = this.active;
    if (!tab?.info.path || tab.ignoreDiskChange) return;
    try {
      if (!(await api.changedOnDisk(tab.id))) return;
      tab.ignoreDiskChange = true;
      const ok = await ask(
        `"${tab.info.name}" was changed by another program.${tab.info.modified ? " Reloading discards your unsaved changes." : ""} Reload it?`,
        { title: "File changed on disk", kind: "warning", okLabel: "Reload", cancelLabel: "Keep mine" },
      );
      if (ok) await this.revertForce(tab);
    } catch {
      /* ignore */
    }
  }

  private async revertForce(tab: Tab) {
    try {
      this.applyInfo(tab, await api.reload(tab.id));
      tab.ignoreDiskChange = false;
    } catch (e) {
      this.error(e);
    }
  }

  /** Called after every change of the document. */
  applyInfo(tab: Tab, info: DocInfo) {
    const sizeChanged = info.size !== tab.info.size;
    const contentChanged = info.stateId !== tab.info.stateId || sizeChanged;
    tab.info = info;
    tab.cache.invalidate(info.size);
    tab.version++;
    if (tab.cursor > info.size) tab.cursor = info.size;
    if (tab.anchor !== null && tab.anchor >= info.size) tab.anchor = info.size > 0 ? info.size - 1 : null;
    if (contentChanged) {
      if (tab.matches.length) {
        tab.matches = [];
        tab.matchIndex = -1;
      }
      this.scheduleAnalysis(tab);
    }
    if (tab === this.active) this.updateTitle();
  }

  private scheduleAnalysis(tab: Tab) {
    clearTimeout(this.analysisTimers.get(tab.id));
    this.analysisTimers.set(tab.id, setTimeout(() => this.analyze(tab), 900));
  }

  analyze(tab: Tab) {
    api.analyzeFormat(tab.id).then((f) => (tab.format = f)).catch(() => {});
    api.entropy(tab.id, 2048).promise.then((e) => (tab.entropy = e)).catch(() => {});
  }

  async withBusy<T>(label: string, f: (progress: (p: number) => void) => Promise<T>, cancel?: () => void): Promise<T> {
    this.busy = { label, progress: -1, cancel };
    try {
      return await f((p) => {
        if (this.busy) this.busy.progress = p;
      });
    } finally {
      this.busy = null;
    }
  }

  // ---- cursor & selection ---------------------------------------------------------

  ensureVisible(tab: Tab, offset = tab.cursor) {
    const { rows, bpr } = this.view;
    const row = Math.floor(offset / bpr);
    if (row < tab.topRow) tab.topRow = row;
    else if (row >= tab.topRow + rows - 1) tab.topRow = Math.max(0, row - rows + 2);
  }

  moveTo(tab: Tab, offset: number, extend = false) {
    const max = tab.insertMode ? tab.info.size : Math.max(0, tab.info.size - 1);
    const pos = Math.max(0, Math.min(max, offset));
    if (extend) {
      if (tab.anchor === null) tab.anchor = tab.cursor;
    } else {
      tab.anchor = null;
    }
    tab.cursor = pos;
    tab.nibble = 0;
    this.ensureVisible(tab);
  }

  /** Selects a range and scrolls it into view (centered if far away). */
  select(tab: Tab, offset: number, len: number) {
    const size = tab.info.size;
    offset = Math.max(0, Math.min(offset, Math.max(0, size - (size > 0 ? 1 : 0))));
    if (len > 1) {
      tab.anchor = offset;
      tab.cursor = Math.min(offset + len - 1, size - 1);
    } else {
      tab.anchor = null;
      tab.cursor = offset;
    }
    tab.nibble = 0;
    this.center(tab, offset);
  }

  center(tab: Tab, offset: number) {
    const { rows, bpr } = this.view;
    const row = Math.floor(offset / bpr);
    if (row < tab.topRow || row >= tab.topRow + rows - 1) {
      tab.topRow = Math.max(0, row - Math.floor(rows / 3));
    }
  }

  selectAll(tab: Tab | null = this.active) {
    if (!tab || tab.info.size === 0) return;
    tab.anchor = 0;
    tab.cursor = tab.info.size - 1;
  }

  // ---- editing --------------------------------------------------------------------

  private guardWritable(tab: Tab): boolean {
    if (tab.info.readonly) {
      this.toast("This file is open read-only. Use Save As to write a copy.");
      return false;
    }
    return true;
  }

  async undo(tab: Tab | null = this.active) {
    if (!tab?.info.canUndo) return;
    try {
      const r = await api.undo(tab.id);
      this.applyInfo(tab, r.info);
      if (r.offset !== null) this.moveTo(tab, r.offset);
    } catch (e) {
      this.error(e);
    }
  }

  async redo(tab: Tab | null = this.active) {
    if (!tab?.info.canRedo) return;
    try {
      const r = await api.redo(tab.id);
      this.applyInfo(tab, r.info);
      if (r.offset !== null) this.moveTo(tab, r.offset);
    } catch (e) {
      this.error(e);
    }
  }

  /** Writes one nibble typed in the hex column. */
  async typeNibble(tab: Tab, digit: number) {
    if (!this.guardWritable(tab)) return;
    const sel = tab.selection;
    try {
      if (sel.len > 0) {
        if (tab.insertMode) this.applyInfo(tab, await api.deleteRange(tab.id, sel.start, sel.len));
        tab.anchor = null;
        tab.cursor = sel.start;
        tab.nibble = 0;
      }
      const pos = tab.cursor;
      if (!tab.insertMode && pos >= tab.info.size && tab.info.size > 0) return;
      // read the live byte: the UI cache may still hold the value from before the last edit
      const current = (await api.readRange(tab.id, pos, 1))[0] ?? 0;
      let info: DocInfo;
      if (tab.nibble === 0) {
        const b = (digit << 4) | (tab.insertMode ? 0 : current & 0x0f);
        info = await api.writeBytes(tab.id, pos, [b], tab.insertMode);
        tab.nibble = 1;
      } else {
        const b = (current & 0xf0) | digit;
        info = await api.writeBytes(tab.id, pos, [b], false);
        tab.nibble = 0;
        tab.cursor = pos + 1;
      }
      this.applyInfo(tab, info);
      if (!tab.insertMode && tab.cursor >= tab.info.size) {
        tab.cursor = Math.max(0, tab.info.size - 1);
        tab.nibble = 0;
      }
      this.ensureVisible(tab);
    } catch (e) {
      this.error(e);
    }
  }

  /** Writes bytes typed in the text column. */
  async typeBytes(tab: Tab, bytes: number[]) {
    if (!this.guardWritable(tab)) return;
    const sel = tab.selection;
    try {
      let pos = tab.cursor;
      if (sel.len > 0) {
        pos = sel.start;
        if (tab.insertMode) this.applyInfo(tab, await api.deleteRange(tab.id, sel.start, sel.len));
        tab.anchor = null;
      }
      if (!tab.insertMode && pos >= tab.info.size && tab.info.size > 0) return;
      this.applyInfo(tab, await api.writeBytes(tab.id, pos, bytes, tab.insertMode));
      tab.cursor = pos + bytes.length;
      tab.nibble = 0;
      if (!tab.insertMode && tab.cursor >= tab.info.size) tab.cursor = Math.max(0, tab.info.size - 1);
      this.ensureVisible(tab);
    } catch (e) {
      this.error(e);
    }
  }

  async deleteBytes(tab: Tab, backward: boolean) {
    if (!this.guardWritable(tab)) return;
    const sel = tab.selection;
    if (!tab.insertMode) {
      if (backward && sel.len === 0) return this.moveTo(tab, tab.cursor - 1);
      this.toast(`Overwrite mode keeps the file size. Press ${insertKey()} for insert mode to delete bytes.`);
      return;
    }
    try {
      if (sel.len > 0) {
        this.applyInfo(tab, await api.deleteRange(tab.id, sel.start, sel.len));
        tab.anchor = null;
        tab.cursor = sel.start;
      } else if (backward) {
        if (tab.cursor === 0) return;
        this.applyInfo(tab, await api.deleteRange(tab.id, tab.cursor - 1, 1));
        tab.cursor -= 1;
      } else {
        if (tab.cursor >= tab.info.size) return;
        this.applyInfo(tab, await api.deleteRange(tab.id, tab.cursor, 1));
      }
      tab.nibble = 0;
      this.ensureVisible(tab);
    } catch (e) {
      this.error(e);
    }
  }

  async copy(tab: Tab | null = this.active, format: "hex" | "text" | null = null, cut = false) {
    if (!tab) return;
    const { start, len } = tab.target;
    if (len === 0) return;
    const fmt = format ?? (tab.pane === "hex" ? "hex" : "text");
    try {
      await api.copyBytes(tab.id, start, len, fmt, settings.data.uppercase);
      if (cut) {
        if (!tab.insertMode) {
          this.toast(`Copied. Cut only removes bytes in insert mode (${insertKey()}).`);
          return;
        }
        if (!this.guardWritable(tab)) return;
        this.applyInfo(tab, await api.deleteRange(tab.id, start, len));
        tab.anchor = null;
        tab.cursor = start;
      } else if (len > 1) {
        this.toast(`Copied ${fmtCount(len)} bytes`, "success", undefined, 1500);
      }
    } catch (e) {
      this.error(e);
    }
  }

  async paste(tab: Tab, text: string, asHex = tab.pane === "hex") {
    if (!this.guardWritable(tab)) return;
    const sel = tab.selection;
    const at = sel.len > 0 ? sel.start : tab.cursor;
    try {
      const r = await api.paste(tab.id, at, sel.len, text, asHex, tab.insertMode);
      this.applyInfo(tab, r.info);
      tab.anchor = null;
      this.moveTo(tab, Math.min(at + r.len, tab.insertMode ? r.info.size : r.info.size - 1));
      if (!tab.insertMode && sel.len === 0 && at + r.len > tab.info.size) {
        this.toast("Pasted data extended the file");
      }
    } catch (e) {
      this.error(e);
    }
  }

  async fill(tab: Tab, pattern: string, len: number, insert: boolean) {
    if (!this.guardWritable(tab)) return false;
    const at = tab.selection.len > 0 ? tab.selection.start : tab.cursor;
    try {
      this.applyInfo(tab, await api.fillRange(tab.id, at, len, pattern, insert));
      this.select(tab, at, len);
      return true;
    } catch (e) {
      this.error(e);
      return false;
    }
  }

  toggleInsert(tab: Tab | null = this.active) {
    if (!tab) return;
    tab.insertMode = !tab.insertMode;
    if (!tab.insertMode && tab.cursor >= tab.info.size) tab.cursor = Math.max(0, tab.info.size - 1);
  }

  // ---- navigation helpers --------------------------------------------------------

  gotoEntry(tab: Tab | null = this.active) {
    if (!tab) return;
    const e = tab.format?.entryOffset;
    if (e == null) return this.toast("No entry point found in this file");
    this.select(tab, e, 1);
  }

  /** Reads a pointer at the cursor and jumps to it (as VA when possible, else as offset). */
  async followPointer(tab: Tab | null = this.active, width: 4 | 8 = 8) {
    if (!tab) return;
    const bytes = await api.readRange(tab.id, tab.cursor, width);
    if (bytes.length < width) return this.toast("Not enough bytes for a pointer here");
    let v = 0n;
    for (let i = 0; i < width; i++) {
      const b = BigInt(bytes[settings.data.littleEndian ? i : width - 1 - i]);
      v |= b << BigInt(8 * i);
    }
    if (v > BigInt(Number.MAX_SAFE_INTEGER)) return this.toast(`0x${v.toString(16)} is not a valid location`);
    const n = Number(v);
    const off = tab.format?.mappings.length ? await api.vaToOffset(tab.id, n) : null;
    if (off != null) {
      this.select(tab, off, 1);
      this.toast(`VA 0x${hex(n)} → offset 0x${hex(off)}`, "info", undefined, 2500);
    } else if (n < tab.info.size) {
      this.select(tab, n, 1);
    } else {
      this.toast(`0x${hex(n)} is outside the file and not a mapped address`);
    }
  }

  // ---- bookmarks ------------------------------------------------------------------

  toggleBookmark(tab: Tab | null = this.active) {
    if (!tab) return;
    const { start, len } = tab.target;
    const hit = tab.bookmarks.findIndex((b) => tab.cursor >= b.offset && tab.cursor < b.offset + Math.max(1, b.len));
    if (hit >= 0 && tab.selection.len === 0) {
      tab.bookmarks.splice(hit, 1);
    } else {
      const color = BOOKMARK_COLORS[tab.bookmarks.length % BOOKMARK_COLORS.length];
      tab.bookmarks.push({
        id: crypto.randomUUID(), offset: start, len: Math.max(1, len), label: `Bookmark ${tab.bookmarks.length + 1}`, color,
      });
      tab.bookmarks.sort((a, b) => a.offset - b.offset);
    }
    this.persistBookmarks(tab);
  }

  jumpBookmark(dir: 1 | -1, tab: Tab | null = this.active) {
    if (!tab || tab.bookmarks.length === 0) return;
    const list = tab.bookmarks;
    const next = dir > 0
      ? list.find((b) => b.offset > tab.cursor) ?? list[0]
      : [...list].reverse().find((b) => b.offset < tab.cursor) ?? list[list.length - 1];
    this.select(tab, next.offset, next.len);
  }

  persistBookmarks(tab: Tab) {
    clearTimeout(this.bookmarkTimers.get(tab.id));
    this.bookmarkTimers.set(
      tab.id,
      setTimeout(() => {
        if (tab.info.path) api.saveBookmarks(tab.id, $state.snapshot(tab.bookmarks)).catch(() => {});
      }, 400),
    );
  }

  // ---- search ---------------------------------------------------------------------

  buildQuery(): Query | null {
    const s = this.search;
    switch (s.mode) {
      case "hex":
        return s.hex.trim() ? { kind: "hex", pattern: s.hex } : null;
      case "text":
        return s.text ? { kind: "text", text: s.text, encoding: s.encoding, caseSensitive: s.caseSensitive } : null;
      case "regex":
        return s.regex ? { kind: "regex", pattern: s.regex, caseSensitive: s.caseSensitive } : null;
      case "number":
        return s.number.trim()
          ? { kind: "number", value: s.number, numType: s.numType, littleEndian: settings.data.littleEndian }
          : null;
    }
  }

  async findNext(backward = false, tab: Tab | null = this.active) {
    if (!tab) return;
    const q = this.buildQuery();
    if (!q) {
      this.openSearch();
      return;
    }
    const sel = tab.selection;
    const from = backward ? (sel.len > 0 ? sel.start : tab.cursor) : sel.len > 0 ? sel.start + 1 : tab.cursor + 1;
    const t = api.find(tab.id, q, from, backward, this.search.wrap);
    const slow = setTimeout(() => (this.busy = { label: "Searching…", progress: -1, cancel: t.cancel }), 300);
    try {
      const m = await t.promise;
      if (m) {
        this.select(tab, m.offset, m.len);
        const i = tab.matches.findIndex((x) => x.offset === m.offset);
        if (i >= 0) tab.matchIndex = i;
      } else {
        this.toast("No match found");
      }
    } catch (e) {
      this.error(e);
    } finally {
      clearTimeout(slow);
      this.busy = null;
    }
  }

  openSearch() {
    settings.set("sidebarOpen", true);
    settings.set("sidebarPanel", "search");
    this.focusSearch++;
  }

  // ---- compare --------------------------------------------------------------------

  async compareWith(tab: Tab | null = this.active) {
    if (!tab) return;
    const picked = await openDialog({ multiple: false, directory: false, title: `Compare ${tab.info.name} with…` });
    if (!picked || Array.isArray(picked)) return;
    try {
      const info = await api.openFile(picked, true);
      if (tab.compare) api.closeDocument(tab.compare.info.id).catch(() => {});
      const cache = new ByteCache(info.id, () => tab.version++);
      cache.size = info.size;
      tab.compare = { info, cache, diffCount: null };
      const t = api.countDifferences(tab.id, info.id);
      t.promise
        .then((n) => {
          if (tab.compare?.info.id === info.id) tab.compare = { ...tab.compare, diffCount: n };
        })
        .catch(() => {});
    } catch (e) {
      this.error(e);
    }
  }

  closeCompare(tab: Tab | null = this.active) {
    if (!tab?.compare) return;
    api.closeDocument(tab.compare.info.id).catch(() => {});
    tab.compare = null;
  }

  async jumpDifference(dir: 1 | -1, tab: Tab | null = this.active) {
    if (!tab?.compare) return this.toast("Use Compare With… first");
    const t = api.nextDifference(tab.id, tab.compare.info.id, dir > 0 ? tab.cursor + 1 : tab.cursor, dir < 0);
    try {
      const off = await t.promise;
      if (off === null) return this.toast(dir > 0 ? "No more differences" : "No earlier differences");
      this.select(tab, off, 1);
    } catch (e) {
      this.error(e);
    }
  }
}

export function insertKey() {
  return /Mac/.test(navigator.platform) ? "⌘I" : "Ctrl+I";
}

export const store = new Store();
