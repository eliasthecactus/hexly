import { api } from "./api";

const PAGE = 64 * 1024;
const MAX_PAGES = 256; // 16 MiB per document

/**
 * Page cache of document bytes on the UI side. The backend owns the data;
 * this only keeps what is (or was recently) on screen.
 */
export class ByteCache {
  private pages = new Map<number, Uint8Array>();
  private pending = new Map<number, number>(); // page -> generation
  private generation = 0;
  size = 0;

  constructor(
    public doc: number,
    private onLoad: () => void,
  ) {}

  /** Drops everything, e.g. after an edit. Keeps showing old data until reloaded. */
  invalidate(size: number) {
    this.generation++;
    this.size = size;
    this.stale = this.pages;
    this.pages = new Map();
    this.pending.clear();
  }

  private stale = new Map<number, Uint8Array>();

  /** Requests the pages covering `[start, start+len)`. */
  ensure(start: number, len: number) {
    if (len <= 0) return;
    const first = Math.floor(start / PAGE);
    const last = Math.floor((Math.min(start + len, this.size) - 1) / PAGE);
    for (let p = first; p <= last; p++) {
      if (this.pages.has(p) || this.pending.get(p) === this.generation) continue;
      const gen = this.generation;
      this.pending.set(p, gen);
      api
        .readRange(this.doc, p * PAGE, PAGE)
        .then((bytes) => {
          if (gen !== this.generation) return;
          this.pending.delete(p);
          this.pages.set(p, bytes);
          this.stale.delete(p);
          if (this.pages.size > MAX_PAGES) {
            const oldest = this.pages.keys().next().value;
            if (oldest !== undefined && oldest !== p) this.pages.delete(oldest);
          }
          this.onLoad();
        })
        .catch(() => {
          if (gen === this.generation) this.pending.delete(p);
        });
    }
  }

  /** Byte at `offset`, `undefined` if not loaded yet, `-1` past the end. */
  get(offset: number): number | undefined {
    if (offset >= this.size) return -1;
    const p = Math.floor(offset / PAGE);
    const page = this.pages.get(p) ?? this.stale.get(p);
    if (!page) return undefined;
    const i = offset - p * PAGE;
    return i < page.length ? page[i] : undefined;
  }

  /** Copies up to `len` loaded bytes at `offset` (missing bytes are undefined). */
  slice(offset: number, len: number): (number | undefined)[] {
    const out: (number | undefined)[] = [];
    for (let i = 0; i < len && offset + i < this.size; i++) out.push(this.get(offset + i));
    return out;
  }
}
