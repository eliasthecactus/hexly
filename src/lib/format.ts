// Pure helpers (unit tested in format.test.ts).

export type TextMode = "ascii" | "latin1" | "ebcdic";

export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

const HEX_UPPER = Array.from({ length: 256 }, (_, i) => i.toString(16).toUpperCase().padStart(2, "0"));
const HEX_LOWER = HEX_UPPER.map((s) => s.toLowerCase());

export function hexByte(b: number, upper = true): string {
  return (upper ? HEX_UPPER : HEX_LOWER)[b];
}

export function hex(n: number, pad = 0, upper = true): string {
  let s = n.toString(16);
  if (upper) s = s.toUpperCase();
  return s.padStart(pad, "0");
}

/** Offset formatted for display, e.g. `0x0001F2A0`. */
export function fmtOffset(n: number, base: "hex" | "dec" = "hex", digits = 8): string {
  if (base === "dec") return n.toLocaleString("en-US");
  return "0x" + hex(n, digits);
}

/** Number of hex digits needed for offsets in a file of `size` bytes. */
export function offsetDigits(size: number): number {
  return Math.max(8, hex(Math.max(size, 1)).length);
}

export function fmtSize(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(2) : v < 100 ? v.toFixed(1) : v.toFixed(0)} ${units[i]}`;
}

/** Byte count with both hex and decimal, e.g. `0x10 (16)`. */
export function fmtCount(n: number): string {
  return `0x${hex(n)} (${n.toLocaleString("en-US")})`;
}

// EBCDIC code page 037 → Unicode for the printable range.
const EBCDIC_037 =
  "\0\x01\x02\x03\x9c\t\x86\x7f\x97\x8d\x8e\x0b\x0c\r\x0e\x0f\x10\x11\x12\x13\x9d\x85\x08\x87\x18\x19\x92\x8f\x1c\x1d\x1e\x1f" +
  "\x80\x81\x82\x83\x84\n\x17\x1b\x88\x89\x8a\x8b\x8c\x05\x06\x07\x90\x91\x16\x93\x94\x95\x96\x04\x98\x99\x9a\x9b\x14\x15\x9e\x1a" +
  " \xa0âäàáãåçñ¢.<(+|&éêëèíîïìß!$*);¬-/ÂÄÀÁÃÅÇÑ¦,%_>?øÉÊËÈÍÎÏÌ`:#@'=\"" +
  "Øabcdefghi«»ðýþ±°jklmnopqrªºæ¸Æ¤µ~stuvwxyz¡¿ÐÝÞ®^£¥·©§¶¼½¾[]¯¨´×" +
  "{ABCDEFGHI\xadôöòóõ}JKLMNOPQR¹ûüùúÿ\\÷STUVWXYZ²ÔÖÒÓÕ0123456789³ÛÜÙÚ\x9f";

/** Character shown in the text column for byte `b`, or null if not printable. */
export function byteChar(b: number, mode: TextMode): string | null {
  if (mode === "ebcdic") {
    const c = EBCDIC_037[b];
    const code = c.charCodeAt(0);
    if (code < 0x20 || (code >= 0x7f && code <= 0xa0) || code === 0xad) return null;
    return c;
  }
  if (b >= 0x20 && b < 0x7f) return String.fromCharCode(b);
  if (mode === "latin1" && b > 0xa0 && b !== 0xad) return String.fromCharCode(b);
  return null;
}

/** Encodes a typed character for the text column; null if not representable. */
export function charToByte(ch: string, mode: TextMode): number | null {
  if (ch.length !== 1) return null;
  if (mode === "ebcdic") {
    const i = EBCDIC_037.indexOf(ch);
    return i >= 0 ? i : null;
  }
  const c = ch.charCodeAt(0);
  if (mode === "ascii") return c < 0x80 ? c : null;
  return c < 0x100 ? c : null;
}

export type ByteClass = "zero" | "printable" | "space" | "control" | "high" | "ff";

export function byteClass(b: number): ByteClass {
  if (b === 0) return "zero";
  if (b === 0xff) return "ff";
  if (b > 0x20 && b < 0x7f) return "printable";
  if (b === 0x20 || b === 0x09 || b === 0x0a || b === 0x0d) return "space";
  if (b < 0x20 || b === 0x7f) return "control";
  return "high";
}

/** Shortcut string like `Mod+Shift+K` → human label (`⌘⇧K` or `Ctrl+Shift+K`). */
export function shortcutLabel(s: string, mac = isMac): string {
  const parts = s.split("+");
  if (mac) {
    const map: Record<string, string> = {
      Mod: "⌘", Cmd: "⌘", Ctrl: "⌃", Shift: "⇧", Alt: "⌥",
      Up: "↑", Down: "↓", Left: "←", Right: "→", Enter: "↩", Escape: "⎋", Tab: "⇥",
    };
    return parts.map((p) => map[p] ?? p.toUpperCase()).join("");
  }
  return parts.map((p) => (p === "Mod" ? "Ctrl" : p)).join("+");
}

/** Does keyboard event `e` match shortcut `s` (e.g. `Mod+Shift+F`)? */
export function matchShortcut(e: KeyboardEvent, s: string, mac = isMac): boolean {
  const parts = s.split("+");
  const key = parts[parts.length - 1];
  // Off macOS, Ctrl is the command modifier, so `Ctrl` and `Mod` mean the same key.
  const want = new Set(parts.slice(0, -1).map((p) => (!mac && p === "Ctrl" ? "Mod" : p)));
  const mod = mac ? e.metaKey : e.ctrlKey;
  if (want.has("Mod") !== mod) return false;
  if (mac && want.has("Ctrl") !== e.ctrlKey) return false;
  if (!mac && e.metaKey) return false;
  if (want.has("Shift") !== e.shiftKey) return false;
  if (want.has("Alt") !== e.altKey) return false;
  const k = e.key;
  const names: Record<string, string> = { Up: "ArrowUp", Down: "ArrowDown", Left: "ArrowLeft", Right: "ArrowRight" };
  if (names[key]) return k === names[key];
  if (key.length === 1) {
    // e.code keeps working with Alt/Shift held and non-US layouts
    if (/^[A-Z]$/i.test(key)) return e.code === `Key${key.toUpperCase()}`;
    if (/^[0-9]$/.test(key)) return e.code === `Digit${key}` || k === key;
    if (key === "=") return k === "=" || k === "+" || e.code === "Equal";
    if (key === "-") return k === "-" || e.code === "Minus";
    if (key === ",") return k === "," || e.code === "Comma";
    if (key === "/") return k === "/" || e.code === "Slash";
    return k === key;
  }
  return k === key;
}

/** Binary search: index of the first item with `key(item) >= value`. */
export function lowerBound<T>(arr: T[], value: number, key: (t: T) => number): number {
  let lo = 0;
  let hi = arr.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (key(arr[mid]) < value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Parses a user-entered hex string (`DE AD`, `dead`) into bytes, or null. */
export function parseHex(s: string): number[] | null {
  const clean = s.replace(/0x/gi, "").replace(/[\s,:-]/g, "");
  if (clean.length === 0 || clean.length % 2 || /[^0-9a-f]/i.test(clean)) return null;
  const out: number[] = [];
  for (let i = 0; i < clean.length; i += 2) out.push(parseInt(clean.slice(i, i + 2), 16));
  return out;
}

export function basename(p: string): string {
  return p.split(/[\\/]/).pop() || p;
}
