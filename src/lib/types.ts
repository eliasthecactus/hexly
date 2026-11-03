export interface DocInfo {
  id: number;
  path: string | null;
  name: string;
  size: number;
  readonly: boolean;
  modified: boolean;
  canUndo: boolean;
  canRedo: boolean;
  stateId: number;
}

export interface Match {
  offset: number;
  len: number;
}

export type TextEncoding = "utf8" | "utf16le" | "utf16be" | "latin1";

export type NumType =
  | "u8" | "i8" | "u16" | "i16" | "u24" | "u32" | "i32" | "u64" | "i64"
  | "f16" | "bf16" | "f32" | "f64" | "uleb128" | "sleb128" | "binary";

export type Query =
  | { kind: "hex"; pattern: string }
  | { kind: "text"; text: string; encoding: TextEncoding; caseSensitive: boolean }
  | { kind: "regex"; pattern: string; caseSensitive: boolean }
  | { kind: "number"; value: string; numType: NumType; littleEndian: boolean };

export interface Field {
  key: string;
  label: string;
  value: string;
  size: number;
  editable: boolean;
}

export type Arch =
  | "x86" | "x86_64" | "arm" | "thumb" | "arm64" | "riscv32" | "riscv64"
  | "mips32" | "mips32le" | "mips64" | "mips64le" | "ppc32" | "ppc64" | "ppc64le";

export interface TreeNode {
  label: string;
  value?: string;
  offset?: number;
  size?: number;
  children?: TreeNode[];
}

export interface Region {
  name: string;
  kind: "header" | "section" | "segment" | "slice" | "overlay";
  offset: number;
  size: number;
  vaddr?: number;
  flags?: string;
}

export interface Mapping {
  vaddr: number;
  vsize: number;
  offset: number;
  fsize: number;
}

export interface Symbol {
  name: string;
  vaddr: number;
  offset?: number;
  size: number;
  kind: string;
}

export interface FormatInfo {
  kind: string;
  description: string;
  arch: Arch | null;
  entry: number | null;
  entryOffset: number | null;
  regions: Region[];
  mappings: Mapping[];
  symbols: Symbol[];
  tree: TreeNode[];
}

export interface Insn {
  offset: number;
  address: number;
  size: number;
  bytes: string;
  mnemonic: string;
  operands: string;
  target?: number;
}

export interface FoundString {
  offset: number;
  len: number;
  encoding: "ascii" | "utf16le";
  text: string;
}

export interface EntropyMap {
  blockSize: number;
  entropy: number[];
  class: number[];
}

export interface HashResult {
  name: string;
  value: string;
}

export type CopyFormat =
  | "hex" | "hexCompact" | "text" | "c" | "rust" | "python" | "go" | "java" | "base64" | "escapedString";

export interface Bookmark {
  id: string;
  offset: number;
  len: number;
  label: string;
  color: string;
}
