<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { store, type Tab, type Pane } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { byteChar, byteClass, charToByte, hex, hexByte, isMac, lowerBound, offsetDigits } from "../format";
  import type { ByteCache } from "../cache";

  interface Props {
    tab: Tab;
    /** Which document this view shows: the tab's own, or the one it is compared with. */
    source?: "main" | "compare";
    oncontext?: (x: number, y: number) => void;
  }
  let { tab, source = "main", oncontext }: Props = $props();

  let host: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let width = $state(0);
  let height = $state(0);
  let focused = $state(false);
  let fontReady = $state(false);

  const isMain = $derived(source === "main");
  const cache = $derived<ByteCache | null>(isMain ? tab.cache : (tab.compare?.cache ?? null));
  const other = $derived<ByteCache | null>(isMain ? (tab.compare?.cache ?? null) : tab.cache);
  const size = $derived(isMain ? tab.info.size : (tab.compare?.info.size ?? 0));

  // ---- layout -----------------------------------------------------------------
  let cw = $state(8);
  const fontSize = $derived(settings.data.fontSize);
  const rowH = $derived(Math.round(fontSize * 1.62));
  const headerH = $derived(rowH + 6);
  const padX = 14;
  const digits = $derived(
    settings.data.offsetBase === "hex"
      ? offsetDigits(Math.max(tab.info.size, tab.compare?.info.size ?? 0))
      : Math.max(8, String(Math.max(tab.info.size, 1)).length),
  );

  function layoutFor(bpr: number) {
    const offX = padX + 8;
    const hexX = offX + (digits + 3) * cw;
    const groups = Math.ceil(bpr / 8);
    const hexW = bpr * 3 * cw + (groups - 1) * cw - cw;
    const textX = hexX + hexW + 3 * cw;
    const total = textX + bpr * cw + padX;
    return { offX, hexX, hexW, textX, total };
  }

  const bpr = $derived.by(() => {
    const s = settings.data.bytesPerRow;
    if (s !== "auto" && (width === 0 || layoutFor(s).total <= width)) return s;
    // auto, or the chosen width doesn't fit (narrow window, side-by-side compare)
    let best = 8;
    for (const n of [8, 16, 24, 32, 40, 48, 56, 64]) {
      if (s !== "auto" && n > s) break;
      if (layoutFor(n).total <= width) best = n;
    }
    return best;
  });
  const L = $derived(layoutFor(bpr));
  const rows = $derived(Math.max(1, Math.floor((height - headerH) / rowH)));
  const totalRows = $derived(Math.floor(Math.max(size, tab.info.size) / bpr) + 1);
  const maxTop = $derived(Math.max(0, totalRows - rows + 1));

  $effect(() => {
    if (isMain) store.view = { rows, bpr };
  });

  // keep topRow in range when the layout changes
  $effect(() => {
    const m = maxTop;
    untrack(() => {
      if (tab.topRow > m) tab.topRow = m;
    });
  });

  const hexByteX = (i: number) => L.hexX + i * 3 * cw + Math.floor(i / 8) * cw;
  const textByteX = (i: number) => L.textX + i * cw;

  // ---- drawing --------------------------------------------------------------------
  let frame = 0;
  function schedule() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      draw();
    });
  }

  $effect(() => {
    // register every dependency the drawing uses
    void [
      tab.cursor, tab.anchor, tab.topRow, tab.nibble, tab.pane, tab.insertMode, tab.version, tab.matches,
      tab.matchIndex, tab.highlight, tab.format, tab.compare, tab.info.size, focused, width, height, cw, fontReady,
      bpr, rows, settings.data.uppercase, settings.data.colorBytes, settings.data.textMode, settings.data.offsetBase,
      settings.data.showSections, settings.data.theme, fontSize, store.dragging,
    ];
    for (const b of tab.bookmarks) void [b.offset, b.len, b.color];
    schedule();
  });

  function cssVars() {
    const s = getComputedStyle(document.documentElement);
    const v = (n: string) => s.getPropertyValue(n).trim();
    return {
      bg: v("--hx-bg"), alt: v("--hx-row-alt"), gutter: v("--hx-gutter"), header: v("--hx-header"),
      text: v("--hx-text"), zero: v("--hx-zero"), printable: v("--hx-printable"), space: v("--hx-space"),
      control: v("--hx-control"), high: v("--hx-high"), ff: v("--hx-ff"), dim: v("--hx-dim"),
      sel: v("--hx-sel"), selOther: v("--hx-sel-other"), match: v("--hx-match"), matchCur: v("--hx-match-current"),
      diff: v("--hx-diff"), diffText: v("--hx-diff-text"), highlight: v("--hx-highlight"), cursor: v("--cursor"),
      cursorText: v("--hx-cursor-text"), accent: v("--accent"), border: v("--border"),
    };
  }

  const SECTION_COLORS = ["#8b5cf6", "#22d3ee", "#f59e0b", "#22c55e", "#ec4899", "#3b82f6", "#ef4444", "#14b8a6"];

  function withAlpha(color: string, a: number) {
    if (!color.startsWith("#") || color.length !== 7) return color;
    const n = parseInt(color.slice(1), 16);
    return `rgba(${n >> 16}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
  }

  // flags per visible byte
  const F_SEL = 1, F_MATCH = 2, F_MATCH_CUR = 4, F_HL = 8, F_DIFF = 16;

  function draw() {
    if (!canvas || !cache) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.max(1, Math.floor(width));
    const H = Math.max(1, Math.floor(height));
    if (canvas.width !== W * dpr || canvas.height !== H * dpr) {
      canvas.width = W * dpr;
      canvas.height = H * dpr;
    }
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const c = cssVars();
    const upper = settings.data.uppercase;
    const mode = settings.data.textMode;
    const colorBytes = settings.data.colorBytes;

    ctx.fillStyle = c.bg;
    ctx.fillRect(0, 0, W, H);
    ctx.font = `${fontSize}px ${getComputedStyle(document.documentElement).getPropertyValue("--mono")}`;
    ctx.textBaseline = "middle";

    const start = tab.topRow * bpr;
    const count = rows * bpr;
    cache.ensure(Math.max(0, start - count), count * 3);
    other?.ensure(Math.max(0, start - count), count * 3);

    // --- flags
    const flags = new Uint8Array(count + bpr);
    const sel = tab.selection;
    if (sel.len > 0) {
      for (let o = Math.max(sel.start, start); o < Math.min(sel.end, start + count); o++) flags[o - start] |= F_SEL;
    }
    const ms = tab.matches;
    if (isMain && ms.length) {
      let i = lowerBound(ms, start, (m) => m.offset + m.len);
      for (; i < ms.length && ms[i].offset < start + count; i++) {
        const m = ms[i];
        const f = i === tab.matchIndex ? F_MATCH_CUR : F_MATCH;
        for (let o = Math.max(m.offset, start); o < Math.min(m.offset + m.len, start + count); o++) flags[o - start] |= f;
      }
    }
    const hl = tab.highlight;
    if (hl) {
      for (let o = Math.max(hl.offset, start); o < Math.min(hl.offset + hl.len, start + count); o++) flags[o - start] |= F_HL;
    }

    const bookmarkColor = (o: number): string | null => {
      for (const b of tab.bookmarks) if (o >= b.offset && o < b.offset + b.len) return b.color;
      return null;
    };

    // --- header
    ctx.fillStyle = c.bg;
    ctx.fillRect(0, 0, W, headerH);
    const curCol = tab.cursor % bpr;
    ctx.fillStyle = c.header;
    ctx.fillText(settings.data.offsetBase === "hex" ? "Offset (h)" : "Offset (d)", L.offX, headerH / 2);
    for (let i = 0; i < bpr; i++) {
      ctx.fillStyle = i === curCol ? c.cursor : c.header;
      ctx.fillText(hexByte(i & 0xff, upper), hexByteX(i), headerH / 2);
    }
    ctx.fillStyle = c.header;
    ctx.fillText(mode === "ascii" ? "ASCII" : mode === "latin1" ? "Latin-1" : "EBCDIC", L.textX, headerH / 2);
    ctx.fillStyle = c.border;
    ctx.fillRect(0, headerH - 1, W, 1);

    ctx.save();
    ctx.beginPath();
    ctx.rect(0, headerH, W, H - headerH);
    ctx.clip();

    const regions = settings.data.showSections && isMain ? tab.format?.regions : undefined;

    for (let r = 0; r < rows; r++) {
      const rowStart = start + r * bpr;
      if (rowStart > Math.max(size, 0) && !(rowStart === 0)) break;
      const y = headerH + r * rowH;
      const cy = y + rowH / 2 + 0.5;
      const row = tab.topRow + r;
      if (row % 2 === 1) {
        ctx.fillStyle = c.alt;
        ctx.fillRect(0, y, W, rowH);
      }

      // section stripe
      if (regions && regions.length) {
        let best = -1;
        for (let i = 0; i < regions.length; i++) {
          const g = regions[i];
          if (rowStart >= g.offset && rowStart < g.offset + g.size && (best < 0 || g.size <= regions[best].size)) best = i;
        }
        if (best >= 0) {
          ctx.fillStyle = SECTION_COLORS[best % SECTION_COLORS.length];
          ctx.globalAlpha = 0.75;
          ctx.fillRect(padX, y + 1, 3, rowH - 2);
          ctx.globalAlpha = 1;
        }
      }

      // offset
      const cursorRow = Math.floor(tab.cursor / bpr) === row;
      ctx.fillStyle = cursorRow ? c.text : c.gutter;
      const off = settings.data.offsetBase === "hex" ? hex(rowStart, digits, upper) : String(rowStart).padStart(digits, " ");
      ctx.fillText(off, L.offX, cy);

      for (let i = 0; i < bpr; i++) {
        const o = rowStart + i;
        const b = cache.get(o);
        if (b === -1) break;
        const fl = flags[o - start];
        const hx = hexByteX(i);
        const tx = textByteX(i);
        let diff = false;
        if (other && b !== undefined) {
          const ob = other.get(o);
          diff = ob !== undefined && ob !== b;
        }

        // backgrounds
        let bg: string | null = null;
        if (fl & F_SEL) bg = focused || !isMain ? c.sel : c.selOther;
        else if (fl & F_MATCH_CUR) bg = c.matchCur;
        else if (fl & F_MATCH) bg = c.match;
        else if (fl & F_HL) bg = c.highlight;
        else if (diff) bg = c.diff;
        else {
          const bc = tab.bookmarks.length ? bookmarkColor(o) : null;
          if (bc) bg = withAlpha(bc, 0.28);
        }
        if (bg) {
          const next = i + 1 < bpr ? flags[o + 1 - start] : 0;
          const joined = (fl & F_SEL) && (next & F_SEL) && (i + 1) % 8 !== 0;
          ctx.fillStyle = bg;
          ctx.fillRect(hx - cw * 0.35, y + 1, joined ? 3 * cw : 2.7 * cw, rowH - 2);
          ctx.fillRect(tx, y + 1, cw, rowH - 2);
        }

        if (b === undefined) {
          ctx.fillStyle = c.dim;
          ctx.fillText("··", hx, cy);
          continue;
        }
        let color = c.text;
        if (diff) color = c.diffText;
        else if (colorBytes) {
          const k = byteClass(b);
          color = k === "zero" ? c.zero : k === "printable" ? c.printable : k === "space" ? c.space : k === "control" ? c.control : k === "ff" ? c.ff : c.high;
        }
        ctx.fillStyle = color;
        ctx.fillText(hexByte(b, upper), hx, cy);
        const ch = byteChar(b, mode);
        ctx.fillStyle = ch === null ? c.dim : diff ? c.diffText : c.text;
        ctx.fillText(ch ?? "·", tx, cy);
      }
    }

    // cursor
    const cr = Math.floor(tab.cursor / bpr) - tab.topRow;
    if (cr >= 0 && cr < rows) {
      const i = tab.cursor % bpr;
      const y = headerH + cr * rowH;
      const cy = y + rowH / 2 + 0.5;
      const b = cache.get(tab.cursor);
      const active = isMain && focused;
      const hx = hexByteX(i) + (tab.pane === "hex" ? tab.nibble * cw : 0);
      const tx = textByteX(i);
      const drawCell = (x: number, w: number, isActivePane: boolean, txt: string) => {
        if (isActivePane && active) {
          if (tab.insertMode) {
            ctx.fillStyle = c.cursor;
            ctx.fillRect(x - 1, y + 2, 2, rowH - 4);
          } else {
            ctx.fillStyle = c.cursor;
            ctx.fillRect(x, y + 2, w, rowH - 4);
            ctx.fillStyle = c.cursorText;
            ctx.fillText(txt, x, cy);
          }
        } else {
          ctx.strokeStyle = c.cursor;
          ctx.globalAlpha = isActivePane ? 0.9 : 0.55;
          ctx.lineWidth = 1;
          ctx.strokeRect(x - 0.5, y + 1.5, w + 1, rowH - 3);
          ctx.globalAlpha = 1;
        }
      };
      const bs = b !== undefined && b >= 0 ? hexByte(b, upper) : "  ";
      if (tab.pane === "hex") {
        drawCell(hx, cw, true, bs[tab.nibble]);
        drawCell(tx, cw, false, "");
      } else {
        drawCell(hexByteX(i), 2 * cw, false, "");
        const ch = b !== undefined && b >= 0 ? (byteChar(b, mode) ?? "·") : " ";
        drawCell(tx, cw, true, ch);
      }
    }
    ctx.restore();

    // column separators
    ctx.fillStyle = c.border;
    ctx.fillRect(L.hexX - 1.5 * cw, 0, 1, H);
    ctx.fillRect(L.textX - 1.5 * cw, 0, 1, H);
  }

  // ---- hit testing ------------------------------------------------------------------
  function hit(x: number, y: number, clampRows = false): { offset: number; pane: Pane; nibble: number } | null {
    let r = Math.floor((y - headerH) / rowH);
    if (clampRows) r = Math.max(0, Math.min(rows - 1, r));
    else if (y < headerH) return null;
    const rowStart = (tab.topRow + r) * bpr;
    let pane: Pane;
    let i: number;
    let nibble = 0;
    if (x >= L.textX - cw) {
      pane = "text";
      i = Math.floor((x - L.textX) / cw);
    } else {
      pane = "hex";
      i = 0;
      for (let k = 0; k < bpr; k++) {
        if (x >= hexByteX(k) - cw * 0.5) i = k;
      }
      nibble = x >= hexByteX(i) + cw ? 1 : 0;
      if (x < L.hexX - cw) i = 0;
    }
    i = Math.max(0, Math.min(bpr - 1, i));
    const maxOff = tab.insertMode && isMain ? tab.info.size : Math.max(0, tab.info.size - 1);
    return { offset: Math.min(rowStart + i, maxOff), pane, nibble };
  }

  let drag: { start: number } | null = null;
  let autoScroll = 0;
  let lastMouse = { x: 0, y: 0 };

  function onMouseDown(e: MouseEvent) {
    canvas.focus();
    if (e.button !== 0) return;
    const rect = canvas.getBoundingClientRect();
    const h = hit(e.clientX - rect.left, e.clientY - rect.top);
    if (!h) return;
    if (e.shiftKey) {
      if (tab.anchor === null) tab.anchor = tab.cursor;
      tab.cursor = h.offset;
    } else {
      tab.anchor = null;
      tab.cursor = h.offset;
      tab.nibble = h.pane === "hex" ? h.nibble : 0;
      drag = { start: h.offset };
    }
    tab.pane = h.pane;
    if (h.pane === "text") tab.nibble = 0;
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp, { once: true });
  }

  function dragTo(x: number, y: number) {
    if (!drag) return;
    const h = hit(x, y, true);
    if (!h) return;
    if (h.offset !== drag.start || tab.anchor !== null) {
      tab.anchor = drag.start;
      tab.cursor = h.offset;
      tab.nibble = 0;
    }
  }

  function onMouseMove(e: MouseEvent) {
    const rect = canvas.getBoundingClientRect();
    lastMouse = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    dragTo(lastMouse.x, lastMouse.y);
    const above = lastMouse.y < headerH;
    const below = lastMouse.y > rect.height;
    if ((above || below) && !autoScroll) {
      autoScroll = window.setInterval(() => {
        const dir = lastMouse.y < headerH ? -1 : lastMouse.y > height ? 1 : 0;
        if (!dir) return;
        const dist = dir < 0 ? headerH - lastMouse.y : lastMouse.y - height;
        const step = Math.max(1, Math.floor(dist / 20));
        tab.topRow = Math.max(0, Math.min(maxTop, tab.topRow + dir * step));
        dragTo(lastMouse.x, lastMouse.y);
      }, 40);
    } else if (!above && !below && autoScroll) {
      clearInterval(autoScroll);
      autoScroll = 0;
    }
  }

  function onMouseUp() {
    drag = null;
    clearInterval(autoScroll);
    autoScroll = 0;
    window.removeEventListener("mousemove", onMouseMove);
  }

  function onContext(e: MouseEvent) {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const h = hit(e.clientX - rect.left, e.clientY - rect.top);
    if (h) {
      const s = tab.selection;
      const inside = s.len > 0 && h.offset >= s.start && h.offset < s.end;
      if (!inside) {
        tab.anchor = null;
        tab.cursor = h.offset;
        tab.pane = h.pane;
        tab.nibble = 0;
      }
    }
    oncontext?.(e.clientX, e.clientY);
  }

  let wheelAcc = 0;
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const px = e.deltaMode === 1 ? e.deltaY * rowH * 3 : e.deltaMode === 2 ? e.deltaY * rows * rowH : e.deltaY;
    wheelAcc += px;
    const step = Math.trunc(wheelAcc / rowH);
    if (step !== 0) {
      wheelAcc -= step * rowH;
      tab.topRow = Math.max(0, Math.min(maxTop, tab.topRow + step));
    }
  }

  // ---- keyboard -------------------------------------------------------------------------
  let queue: Promise<unknown> = Promise.resolve();
  const enqueue = (f: () => Promise<unknown>) => (queue = queue.then(f, f));

  function onKeyDown(e: KeyboardEvent) {
    if (e.isComposing) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const extend = e.shiftKey;
    const size = tab.info.size;
    const move = (to: number) => {
      e.preventDefault();
      store.moveTo(tab, to, extend);
    };
    switch (e.key) {
      case "ArrowLeft":
        if (e.altKey) return;
        if (isMac && e.metaKey) return move(tab.cursor - (tab.cursor % bpr));
        return move(tab.cursor - 1);
      case "ArrowRight":
        if (e.altKey) return;
        if (isMac && e.metaKey) return move(tab.cursor - (tab.cursor % bpr) + bpr - 1);
        return move(tab.cursor + 1);
      case "ArrowUp":
        if (e.altKey) return;
        if (mod) return move(0);
        return move(tab.cursor - bpr);
      case "ArrowDown":
        if (e.altKey) return;
        if (mod) return move(size);
        return move(tab.cursor + bpr);
      case "PageUp":
        tab.topRow = Math.max(0, tab.topRow - (rows - 1));
        return move(tab.cursor - (rows - 1) * bpr);
      case "PageDown":
        tab.topRow = Math.min(maxTop, tab.topRow + (rows - 1));
        return move(tab.cursor + (rows - 1) * bpr);
      case "Home":
        return move(mod ? 0 : tab.cursor - (tab.cursor % bpr));
      case "End":
        return move(mod ? size : tab.cursor - (tab.cursor % bpr) + bpr - 1);
      case "Tab":
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        e.preventDefault();
        tab.pane = tab.pane === "hex" ? "text" : "hex";
        tab.nibble = 0;
        return;
      case "Escape":
        tab.anchor = null;
        tab.highlight = null;
        return;
    }
    if (!isMain) return;
    if (e.key === "Backspace" || e.key === "Delete") {
      if (mod || e.altKey) return;
      e.preventDefault();
      enqueue(() => store.deleteBytes(tab, e.key === "Backspace"));
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey || e.key.length !== 1) return;
    if (tab.pane === "hex") {
      const d = parseInt(e.key, 16);
      if (Number.isNaN(d) || !/[0-9a-f]/i.test(e.key)) return;
      e.preventDefault();
      enqueue(() => store.typeNibble(tab, d));
    } else {
      const b = charToByte(e.key, settings.data.textMode);
      if (b === null) return;
      e.preventDefault();
      enqueue(() => store.typeBytes(tab, [b]));
    }
  }

  // ---- lifecycle ----------------------------------------------------------------------
  function measure() {
    const ctx = canvas.getContext("2d")!;
    ctx.font = `${fontSize}px ${getComputedStyle(document.documentElement).getPropertyValue("--mono")}`;
    cw = ctx.measureText("0000000000").width / 10;
  }

  $effect(() => {
    void fontSize;
    void fontReady;
    measure();
  });

  onMount(() => {
    const ro = new ResizeObserver(() => {
      width = host.clientWidth;
      height = host.clientHeight;
    });
    ro.observe(host);
    document.fonts.ready.then(() => (fontReady = true));
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onScheme = () => schedule();
    media.addEventListener("change", onScheme);
    if (isMain) canvas.focus();
    return () => {
      ro.disconnect();
      media.removeEventListener("change", onScheme);
      cancelAnimationFrame(frame);
      onMouseUp();
    };
  });

  export function focus() {
    canvas?.focus();
  }
</script>

<div class="hexview" bind:this={host}>
  <canvas
    bind:this={canvas}
    tabindex="0"
    data-hexview={source}
    style:width="{width}px"
    style:height="{height}px"
    onmousedown={onMouseDown}
    oncontextmenu={onContext}
    onwheel={onWheel}
    onkeydown={onKeyDown}
    onfocus={() => (focused = true)}
    onblur={() => (focused = false)}
    aria-label="Hex editor"
  ></canvas>
</div>

<style>
  .hexview {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--hx-bg);
  }
  canvas {
    display: block;
    outline: none;
    cursor: text;
  }
</style>
