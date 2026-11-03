import { describe, expect, it } from "vitest";
import {
  byteChar, byteClass, charToByte, fmtOffset, fmtSize, hex, lowerBound, matchShortcut, offsetDigits,
  parseHex, shortcutLabel,
} from "./format";

function key(init: Partial<KeyboardEvent> & { key: string; code?: string }): KeyboardEvent {
  return {
    metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, code: "", ...init,
  } as KeyboardEvent;
}

describe("format helpers", () => {
  it("formats offsets and sizes", () => {
    expect(hex(255, 4)).toBe("00FF");
    expect(fmtOffset(0x1f2a0)).toBe("0x0001F2A0");
    expect(fmtOffset(1234, "dec")).toBe("1,234");
    expect(offsetDigits(100)).toBe(8);
    expect(offsetDigits(2 ** 40)).toBe(11);
    expect(fmtSize(512)).toBe("512 B");
    expect(fmtSize(1536)).toBe("1.50 KiB");
    expect(fmtSize(5 * 1024 ** 3)).toBe("5.00 GiB");
  });

  it("maps bytes to characters", () => {
    expect(byteChar(0x41, "ascii")).toBe("A");
    expect(byteChar(0x00, "ascii")).toBeNull();
    expect(byteChar(0xe9, "ascii")).toBeNull();
    expect(byteChar(0xe9, "latin1")).toBe("é");
    expect(byteChar(0xc1, "ebcdic")).toBe("A");
    expect(byteChar(0xf0, "ebcdic")).toBe("0");
    expect(charToByte("A", "ebcdic")).toBe(0xc1);
    expect(charToByte("é", "ascii")).toBeNull();
    expect(charToByte("é", "latin1")).toBe(0xe9);
    expect(byteClass(0)).toBe("zero");
    expect(byteClass(0x41)).toBe("printable");
    expect(byteClass(0xff)).toBe("ff");
  });

  it("matches shortcuts per platform", () => {
    expect(matchShortcut(key({ key: "f", code: "KeyF", metaKey: true }), "Mod+F", true)).toBe(true);
    expect(matchShortcut(key({ key: "f", code: "KeyF", ctrlKey: true }), "Mod+F", true)).toBe(false);
    expect(matchShortcut(key({ key: "f", code: "KeyF", ctrlKey: true }), "Mod+F", false)).toBe(true);
    expect(matchShortcut(key({ key: "F", code: "KeyF", ctrlKey: true, shiftKey: true }), "Mod+Shift+F", false)).toBe(true);
    expect(matchShortcut(key({ key: "F", code: "KeyF", ctrlKey: true, shiftKey: true }), "Mod+F", false)).toBe(false);
    expect(matchShortcut(key({ key: "F2", code: "F2" }), "F2", false)).toBe(true);
    expect(matchShortcut(key({ key: "Tab", code: "Tab", ctrlKey: true }), "Ctrl+Tab", false)).toBe(true);
    expect(matchShortcut(key({ key: "Tab", code: "Tab", ctrlKey: true }), "Ctrl+Tab", true)).toBe(true);
    expect(matchShortcut(key({ key: "Tab", code: "Tab", metaKey: true }), "Ctrl+Tab", true)).toBe(false);
    expect(shortcutLabel("Mod+Shift+K", true)).toBe("⌘⇧K");
    expect(shortcutLabel("Mod+Shift+K", false)).toBe("Ctrl+Shift+K");
  });

  it("parses hex and searches sorted arrays", () => {
    expect(parseHex("DE ad 0x01")).toEqual([0xde, 0xad, 0x01]);
    expect(parseHex("abc")).toBeNull();
    const arr = [1, 3, 5, 7];
    expect(lowerBound(arr, 4, (x) => x)).toBe(2);
    expect(lowerBound(arr, 0, (x) => x)).toBe(0);
    expect(lowerBound(arr, 9, (x) => x)).toBe(4);
  });
});
