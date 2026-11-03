import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Arch, Bookmark, CopyFormat, DocInfo, EntropyMap, Field, FormatInfo, FoundString,
  HashResult, Insn, Match, NumType, Query,
} from "./types";

let nextTask = 1;
const progressHandlers = new Map<number, (p: number) => void>();

listen<[number, number]>("task-progress", (e) => {
  const [id, p] = e.payload;
  progressHandlers.get(id)?.(p);
}).catch(() => {});

/** A cancellable long-running backend operation. */
export interface Task<T> {
  id: number;
  promise: Promise<T>;
  cancel: () => void;
}

function task<T>(cmd: string, args: Record<string, unknown>, onProgress?: (p: number) => void): Task<T> {
  const id = nextTask++;
  if (onProgress) progressHandlers.set(id, onProgress);
  const promise = invoke<T>(cmd, { ...args, taskId: id }).finally(() => progressHandlers.delete(id));
  return { id, promise, cancel: () => void invoke("cancel_task", { taskId: id }) };
}

export const api = {
  openFile: (path: string, readonly = false) => invoke<DocInfo>("open_file", { path, readonly }),
  newDocument: () => invoke<DocInfo>("new_document"),
  closeDocument: (doc: number) => invoke<void>("close_document", { doc }),
  docInfo: (doc: number) => invoke<DocInfo>("doc_info", { doc }),
  takePendingOpens: () => invoke<string[]>("take_pending_opens"),
  async readRange(doc: number, offset: number, len: number): Promise<Uint8Array> {
    const buf = await invoke<ArrayBuffer | number[]>("read_range", { doc, offset, len });
    return buf instanceof ArrayBuffer ? new Uint8Array(buf) : new Uint8Array(buf);
  },
  changedOnDisk: (doc: number) => invoke<boolean>("changed_on_disk", { doc }),
  save: (doc: number) => invoke<DocInfo>("save", { doc }),
  saveAs: (doc: number, path: string) => invoke<DocInfo>("save_as", { doc, path }),
  exportRange: (doc: number, offset: number, len: number, path: string) =>
    invoke<void>("export_range", { doc, offset, len, path }),
  reload: (doc: number) => invoke<DocInfo>("reload", { doc }),

  writeBytes: (doc: number, offset: number, data: number[], insert: boolean) =>
    invoke<DocInfo>("write_bytes", { doc, offset, data, insert }),
  replaceRange: (doc: number, offset: number, len: number, data: number[]) =>
    invoke<DocInfo>("replace_range", { doc, offset, len, data }),
  deleteRange: (doc: number, offset: number, len: number) =>
    invoke<DocInfo>("delete_range", { doc, offset, len }),
  fillRange: (doc: number, offset: number, len: number, pattern: string, insert: boolean) =>
    invoke<DocInfo>("fill_range", { doc, offset, len, pattern, insert }),
  undo: (doc: number) => invoke<{ info: DocInfo; offset: number | null }>("undo", { doc }),
  redo: (doc: number) => invoke<{ info: DocInfo; offset: number | null }>("redo", { doc }),

  copyBytes: (doc: number, offset: number, len: number, format: CopyFormat, upper: boolean) =>
    invoke<number>("copy_bytes", { doc, offset, len, format, upper }),
  formatBytes: (doc: number, offset: number, len: number, format: CopyFormat, upper: boolean) =>
    invoke<string>("format_bytes", { doc, offset, len, format, upper }),
  paste: (doc: number, offset: number, replaceLen: number, text: string, asHex: boolean, insert: boolean) =>
    invoke<{ info: DocInfo; len: number }>("paste", { doc, offset, replaceLen, text, asHex, insert }),

  inspect: (doc: number, offset: number, littleEndian: boolean) =>
    invoke<Field[]>("inspect", { doc, offset, littleEndian }),
  writeValue: (doc: number, offset: number, numType: NumType, value: string, littleEndian: boolean) =>
    invoke<DocInfo>("write_value", { doc, offset, numType, value, littleEndian }),

  find: (doc: number, query: Query, from: number, backward: boolean, wrap: boolean, onProgress?: (p: number) => void) =>
    task<Match | null>("find", { doc, query, from, backward, wrap }, onProgress),
  findAll: (doc: number, query: Query, limit: number, onProgress?: (p: number) => void) =>
    task<{ matches: Match[]; truncated: boolean }>("find_all", { doc, query, limit }, onProgress),
  replaceOne: (doc: number, offset: number, len: number, query: Query, replacement: string) =>
    invoke<{ info: DocInfo; len: number }>("replace_one", { doc, offset, len, query, replacement }),
  replaceAll: (doc: number, query: Query, replacement: string, onProgress?: (p: number) => void) =>
    task<{ info: DocInfo; count: number }>("replace_all", { doc, query, replacement }, onProgress),

  analyzeFormat: (doc: number) => invoke<FormatInfo>("analyze_format", { doc }),
  disassemble: (doc: number, offset: number, count: number, arch: Arch | null) =>
    invoke<{ arch: Arch | null; insns: Insn[] }>("disassemble", { doc, offset, count, arch }),
  architectures: () => invoke<Arch[]>("architectures"),
  findStrings: (
    doc: number,
    options: { minLen: number; ascii: boolean; utf16: boolean; limit: number },
    onProgress?: (p: number) => void,
  ) => task<{ strings: FoundString[]; truncated: boolean }>("find_strings", { doc, options }, onProgress),
  entropy: (doc: number, blocks: number) => task<EntropyMap>("entropy", { doc, blocks }),
  hashes: (doc: number, offset: number, len: number, onProgress?: (p: number) => void) =>
    task<HashResult[]>("hashes", { doc, offset, len }, onProgress),
  nextDifference: (a: number, b: number, from: number, backward: boolean) =>
    task<number | null>("next_difference", { a, b, from, backward }),
  countDifferences: (a: number, b: number) => task<number>("count_differences", { a, b }),

  vaToOffset: (doc: number, va: number) => invoke<number | null>("va_to_offset", { doc, va }),
  offsetToVa: (doc: number, offset: number) => invoke<number | null>("offset_to_va", { doc, offset }),
  evalOffset: (
    doc: number,
    expr: string,
    ctx: { cursor: number; selStart: number; selEnd: number; hexDefault: boolean; virtualAddress: boolean },
  ) => invoke<number>("eval_offset", { doc, expr, ...ctx }),

  loadBookmarks: (doc: number) => invoke<Bookmark[]>("load_bookmarks", { doc }),
  saveBookmarks: (doc: number, bookmarks: Bookmark[]) => invoke<void>("save_bookmarks", { doc, bookmarks }),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
