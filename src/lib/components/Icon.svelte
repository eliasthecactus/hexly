<script lang="ts" module>
  const PATHS: Record<string, string> = {
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    save: "M5 3h11l5 5v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z M7 3v5h8V3 M7 21v-7h10v7",
    plus: "M12 5v14 M5 12h14",
    compare: "M4 4h7v16H4z M13 4h7v16h-7z M6.5 9h2 M15.5 9h2 M6.5 13h2 M15.5 13h2",
    search: "M11 4a7 7 0 1 1 0 14a7 7 0 0 1 0-14z M21 21l-5-5",
    command: "M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z",
    sidebar: "M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z M15 4v16",
    inspector: "M7 4v6 M5.5 4H7 M5 10h4 M15 4h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z M17 14v6 M15.5 14H17 M15 20h4 M7 14h0a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2h0a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2z",
    structure: "M4 4h6v4H4z M14 10h6v4h-6z M14 16h6v4h-6z M7 8v10h7 M7 12h7",
    strings: "M4 7V5h16v2 M9 19h6 M12 5v14",
    cpu: "M7 6h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1z M10 10h4v4h-4z M9 2v4 M15 2v4 M9 18v4 M15 18v4 M2 9h4 M2 15h4 M18 9h4 M18 15h4",
    bookmark: "M6 3h12v18l-6-4-6 4z",
    hash: "M5 9h15 M4 15h15 M10 3L8 21 M16 3l-2 18",
    x: "M6 6l12 12 M18 6L6 18",
    "chevron-right": "M9 6l6 6-6 6",
    "chevron-down": "M6 9l6 6 6-6",
    "chevron-up": "M6 15l6-6 6 6",
    sliders: "M4 6h9 M17 6h3 M4 12h3 M11 12h9 M4 18h11 M19 18h1 M15 4v4 M9 10v4 M17 16v4",
    copy: "M10 8h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1z M5 16V5a1 1 0 0 1 1-1h10",
    "arrow-up": "M12 19V5 M6 11l6-6 6 6",
    "arrow-down": "M12 5v14 M6 13l6 6 6-6",
    trash: "M4 7h16 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3",
    refresh: "M20 11a8 8 0 1 0-2.3 5.7 M20 4v7h-7",
    download: "M12 4v11 M7 10l5 5 5-5 M5 20h14",
    target: "M12 4a8 8 0 1 1 0 16a8 8 0 0 1 0-16z M12 9a3 3 0 1 1 0 6a3 3 0 0 1 0-6z M12 1v4 M12 19v4 M1 12h4 M19 12h4",
    file: "M6 3h8l5 5v12a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z M14 3v5h5",
    lock: "M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3",
    check: "M5 12l5 5 9-10",
    info: "M12 4a8 8 0 1 1 0 16a8 8 0 0 1 0-16z M12 11v5 M12 8h0",
    alert: "M12 4l9 16H3z M12 10v4 M12 17h0",
    play: "M8 5v14l11-7z",
    stop: "M7 7h10v10H7z",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1 M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
  };
</script>

<script lang="ts">
  let { name, size = 16, stroke = 1.8 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name] ?? ""} />
</svg>
