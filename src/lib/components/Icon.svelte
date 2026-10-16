<script lang="ts">
  const paths: Record<string, string> = {
    library: "M4 5h4v14H4zM10 5h4v14h-4zM16.5 5.8l3.9-1 3 13.9-3.9 1z",
    discover: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zm3.6 5.4-1.8 5.4-5.4 1.8 1.8-5.4z",
    download: "M12 4v10m0 0-4-4m4 4 4-4M5 19h14",
    settings: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM19.4 13a7.6 7.6 0 0 0 0-2l2-1.5-2-3.5-2.3 1a7 7 0 0 0-1.7-1L15 3.5h-4l-.4 2.5a7 7 0 0 0-1.7 1l-2.3-1-2 3.5 2 1.5a7.6 7.6 0 0 0 0 2l-2 1.5 2 3.5 2.3-1a7 7 0 0 0 1.7 1l.4 2.5h4l.4-2.5a7 7 0 0 0 1.7-1l2.3 1 2-3.5z",
    play: "M8 5v14l11-7z",
    sun: "M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM12 1v3M12 20v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M1 12h3M20 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1",
    moon: "M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm9 16-4.2-4.2",
    check: "M5 12.5l4.5 4.5L19 7.5",
    x: "M6 6l12 12M18 6 6 18",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    tool: "M14.7 6.3a4 4 0 0 0-5.4 5.1L3 17.7 6.3 21l6.3-6.3a4 4 0 0 0 5.1-5.4l-2.6 2.6-2.4-.6-.6-2.4z",
    plus: "M12 5v14M5 12h14",
    trash: "M4 7h16M10 11v6M14 11v6M6 7l1 12h10l1-12M9 7V4h6v3",
    chevron: "M6 9l6 6 6-6",
    image: "M4 5h16v14H4zM4 16l4.5-4.5 3.5 3.5 3-3L20 16M9 9.5h.01",
    collapse: "M15 6l-6 6 6 6",
    expand: "M9 6l6 6-6 6",
    bell: "M6 16v-5a6 6 0 0 1 12 0v5l2 2H4zM10 21h4",
    refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
    edit: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
    undo: "M9 14 4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
  };
  let { name, size = 18 }: { name: keyof typeof paths | string; size?: number } = $props();
  const filled = $derived(name === "play");
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill={filled ? "currentColor" : "none"}
  stroke={filled ? "none" : "currentColor"} stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>
