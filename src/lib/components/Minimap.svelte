<script lang="ts">
  import { onMount } from "svelte";
  import { store, type Tab } from "../store.svelte";
  import { settings } from "../settings.svelte";
  import { fmtOffset } from "../format";

  let { tab }: { tab: Tab } = $props();

  let host: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let width = $state(0);
  let height = $state(0);
  let hover = $state<{ y: number; text: string } | null>(null);

  const W = 58;
  const bpr = $derived(store.view.bpr);
  const rows = $derived(store.view.rows);
  const size = $derived(Math.max(tab.info.size, 1));
  const totalRows = $derived(Math.floor(tab.info.size / bpr) + 1);

  const SECTION_COLORS = ["#8b5cf6", "#22d3ee", "#f59e0b", "#22c55e", "#ec4899", "#3b82f6", "#ef4444", "#14b8a6"];

  // Byte class → base color; entropy adds intensity.
  function blockColor(cls: number, e: number, dark: boolean): string {
    const a = 0.25 + 0.75 * e;
    switch (cls) {
      case 0:
        return dark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.03)";
      case 1:
        return `rgba(96, 165, 250, ${0.35 + 0.5 * e})`;
      case 3:
        return `rgba(${Math.round(244 - 30 * e)}, ${Math.round(114 - 60 * e)}, 94, ${a})`;
      default:
        return `rgba(167, 139, 250, ${a * 0.85})`;
    }
  }

  function draw() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = W * dpr;
    canvas.height = Math.max(1, height) * dpr;
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const css = getComputedStyle(document.documentElement);
    const dark = document.documentElement.dataset.theme !== "light";
    ctx.fillStyle = css.getPropertyValue("--panel").trim();
    ctx.fillRect(0, 0, W, height);

    const H = height;
    const y = (off: number) => (off / size) * H;

    // entropy blocks
    const ent = tab.entropy;
    if (ent && ent.entropy.length) {
      const n = ent.entropy.length;
      for (let i = 0; i < n; i++) {
        const y0 = y(i * ent.blockSize);
        const y1 = Math.max(y0 + 1, y((i + 1) * ent.blockSize));
        ctx.fillStyle = blockColor(ent.class[i], ent.entropy[i] / 255, dark);
        ctx.fillRect(12, y0, W - 20, y1 - y0 + 0.5);
      }
    }

    // regions (sections) on the left
    const regs = tab.format?.regions ?? [];
    regs.forEach((r, i) => {
      if (r.kind === "header" || r.size === 0) return;
      ctx.fillStyle = SECTION_COLORS[i % SECTION_COLORS.length];
      ctx.globalAlpha = 0.85;
      ctx.fillRect(r.kind === "segment" || r.kind === "slice" ? 2 : 6, y(r.offset), 3, Math.max(1, y(r.offset + r.size) - y(r.offset)));
      ctx.globalAlpha = 1;
    });

    // bookmarks and matches as ticks on the right
    for (const b of tab.bookmarks) {
      ctx.fillStyle = b.color;
      ctx.fillRect(W - 7, y(b.offset) - 1, 6, 3);
    }
    if (tab.matches.length) {
      ctx.fillStyle = "rgba(250, 204, 21, 0.9)";
      let last = -10;
      for (const m of tab.matches) {
        const yy = Math.floor(y(m.offset));
        if (yy === last) continue;
        last = yy;
        ctx.fillRect(W - 7, yy, 6, 1.5);
      }
    }

    // viewport
    const vy = (tab.topRow / totalRows) * H;
    const vh = Math.max(6, (rows / totalRows) * H);
    ctx.fillStyle = dark ? "rgba(255,255,255,0.10)" : "rgba(0,0,0,0.08)";
    ctx.fillRect(0, vy, W, vh);
    ctx.strokeStyle = css.getPropertyValue("--accent-2").trim();
    ctx.lineWidth = 1;
    ctx.strokeRect(0.5, vy + 0.5, W - 1, vh - 1);

    // cursor line
    ctx.fillStyle = css.getPropertyValue("--cursor").trim();
    ctx.fillRect(0, y(tab.cursor), W, 1.5);
  }

  $effect(() => {
    void [tab.entropy, tab.format, tab.matches, tab.topRow, tab.cursor, tab.info.size, height, rows, bpr, settings.data.theme];
    for (const b of tab.bookmarks) void [b.offset, b.color];
    draw();
  });

  function rowAt(clientY: number) {
    const rect = canvas.getBoundingClientRect();
    const f = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height));
    return Math.floor(f * totalRows);
  }

  function scrollTo(clientY: number) {
    const row = rowAt(clientY);
    tab.topRow = Math.max(0, Math.min(totalRows - rows + 1, row - Math.floor(rows / 2)));
  }

  function onDown(e: MouseEvent) {
    if (e.button !== 0) return;
    scrollTo(e.clientY);
    const move = (ev: MouseEvent) => scrollTo(ev.clientY);
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", () => window.removeEventListener("mousemove", move), { once: true });
  }

  function onHover(e: MouseEvent) {
    const rect = canvas.getBoundingClientRect();
    const off = Math.min(tab.info.size, rowAt(e.clientY) * bpr);
    const reg = tab.regionAt(off);
    const ent = tab.entropy;
    let extra = "";
    if (ent?.entropy.length) {
      const i = Math.min(ent.entropy.length - 1, Math.floor(off / ent.blockSize));
      extra = ` · entropy ${(ent.entropy[i] / 255 * 8).toFixed(2)} bits`;
    }
    hover = { y: e.clientY - rect.top, text: `${fmtOffset(off)}${reg ? ` · ${reg.name}` : ""}${extra}` };
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const step = Math.sign(e.deltaY) * Math.max(1, Math.round(Math.abs(e.deltaY) / 20)) * Math.max(1, Math.round(totalRows / 400));
    tab.topRow = Math.max(0, Math.min(totalRows - rows + 1, tab.topRow + step));
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      width = host.clientWidth;
      height = host.clientHeight;
    });
    ro.observe(host);
    return () => ro.disconnect();
  });
</script>

<div class="minimap" bind:this={host} style:width="{W}px">
  <canvas
    bind:this={canvas}
    style:width="{W}px"
    style:height="{height}px"
    onmousedown={onDown}
    onmousemove={onHover}
    onmouseleave={() => (hover = null)}
    onwheel={onWheel}
  ></canvas>
  {#if hover}
    <div class="tip mono" style:top="{Math.min(Math.max(hover.y - 12, 4), height - 28)}px">{hover.text}</div>
  {/if}
</div>

<style>
  .minimap {
    position: relative;
    flex: none;
    border-left: 1px solid var(--border);
    background: var(--panel);
  }
  canvas {
    display: block;
    cursor: pointer;
  }
  .tip {
    position: absolute;
    right: calc(100% + 6px);
    white-space: nowrap;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    padding: 3px 8px;
    font-size: 11.5px;
    box-shadow: var(--shadow);
    pointer-events: none;
    z-index: 5;
  }
</style>
