import type { TextMode } from "./format";

export type Theme = "system" | "dark" | "light";
export type BytesPerRow = "auto" | 8 | 16 | 24 | 32 | 48 | 64;

export interface SettingsData {
  theme: Theme;
  fontSize: number;
  bytesPerRow: BytesPerRow;
  uppercase: boolean;
  offsetBase: "hex" | "dec";
  textMode: TextMode;
  colorBytes: boolean;
  showSections: boolean;
  littleEndian: boolean;
  hexDefaultGoto: boolean;
  checkUpdates: boolean;
  sidebarWidth: number;
  sidebarOpen: boolean;
  sidebarPanel: string;
  recent: string[];
}

const DEFAULTS: SettingsData = {
  theme: "system",
  fontSize: 13,
  bytesPerRow: 16,
  uppercase: true,
  offsetBase: "hex",
  textMode: "ascii",
  colorBytes: true,
  showSections: true,
  littleEndian: true,
  hexDefaultGoto: true,
  checkUpdates: true,
  sidebarWidth: 360,
  sidebarOpen: true,
  sidebarPanel: "inspector",
  recent: [],
};

const KEY = "hexly.settings.v1";

function load(): SettingsData {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULTS, ...JSON.parse(raw) };
  } catch {
    /* storage unavailable */
  }
  return { ...DEFAULTS };
}

class Settings {
  data = $state<SettingsData>(load());

  save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.data));
    } catch {
      /* ignore */
    }
  }

  set<K extends keyof SettingsData>(key: K, value: SettingsData[K]) {
    this.data[key] = value;
    this.save();
  }

  addRecent(path: string) {
    const list = [path, ...this.data.recent.filter((p) => p !== path)].slice(0, 12);
    this.set("recent", list);
  }

  removeRecent(path: string) {
    this.set("recent", this.data.recent.filter((p) => p !== path));
  }

  reset() {
    const recent = this.data.recent;
    this.data = { ...DEFAULTS, recent };
    this.save();
  }
}

export const settings = new Settings();
