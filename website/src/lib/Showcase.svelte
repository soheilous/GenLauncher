<script lang="ts">
  import { onMount } from "svelte";

  // The interactive demo: the launcher's own interface (built by tools/build-demo.mjs into static/demo) in a frame at its
  // natural size, scaled to fit, driven by the chapter buttons through messages.
  const W = 1240;
  const H = 780;

  type Command =
    | { type: "theme"; value: "dark" | "light" }
    | { type: "view"; value: "library" | "discover" | "downloads" | "notifications" }
    | { type: "open"; value: "settings" | "options" | "add" | "drop" }
    | { type: "install" }
    | { type: "play" };

  const chapters: { title: string; text: string; icon: string; commands: Command[] }[] = [
    { title: "Library", icon: "M4 5h4v14H4zM10 5h4v14h-4zM16.5 5.8l3.9-1 3 13.9-3.9 1z",
      text: "Your mods with their versions, patches, add-ons and executables. Select a mod, pick a version, switch things on.",
      commands: [{ type: "view", value: "library" }] },
    { title: "Play", icon: "M8 5v14l11-7z",
      text: "Press Play: the launcher prepares your game folder, starts the game and puts everything back when it closes. (Simulated here.)",
      commands: [{ type: "play" }] },
    { title: "Install", icon: "M12 4v10m0 0-4-4m4 4 4-4M5 19h14",
      text: "Press Install in Discover: the mod's own page opens in a built-in browser and the launcher takes the file you download. The demo shows the progress.",
      commands: [{ type: "install" }] },
    { title: "Drop a file", icon: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
      text: "Drag a zip, 7z, rar or folder onto the window, then say whether it is a mod, a patch, an add-on or an executable, and which mod it is for.",
      commands: [{ type: "open", value: "drop" }] },
    { title: "Game options", icon: "M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M10 10v4M16 16v4",
      text: "Resolution, texture quality, particle limit and effects, written to the game's own settings file, with a backup of the original.",
      commands: [{ type: "open", value: "options" }] },
    { title: "Notifications", icon: "M6 16v-5a6 6 0 0 1 12 0v5l2 2H4zM10 21h4",
      text: "Finished installs, problems and messages stay in one place until you dismiss them.",
      commands: [{ type: "view", value: "notifications" }] },
    { title: "Settings", icon: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM19.4 13a7.6 7.6 0 0 0 0-2l2-1.5-2-3.5-2.3 1a7 7 0 0 0-1.7-1L15 3.5h-4l-.4 2.5a7 7 0 0 0-1.7 1l-2.3-1-2 3.5 2 1.5a7.6 7.6 0 0 0 0 2l-2 1.5 2 3.5 2.3-1a7 7 0 0 0 1.7 1l.4 2.5h4l.4-2.5a7 7 0 0 0 1.7-1l2.3 1 2-3.5z",
      text: "13 languages (try switching), dark and light themes, backups and Google Drive sync.",
      commands: [{ type: "open", value: "settings" }] },
  ];

  let width = $state(W);
  const scale = $derived(Math.min(1, width / W));
  let frame = $state<HTMLIFrameElement>();
  let src = $state("");
  let ready = $state(false);
  let active = $state(0);
  let theme = $state<"dark" | "light">("dark");
  let pending: Command[] = [];
  let loads = 0;

  const demoUrl = () => `/demo/?theme=${theme}${loads ? `&r=${loads}` : ""}`;

  function post(c: Command) {
    if (!ready) { pending.push(c); return; }
    frame?.contentWindow?.postMessage({ source: "genlauncher-site", command: c }, location.origin);
  }

  function choose(i: number) {
    active = i;
    for (const c of chapters[i].commands) post(c);
  }

  /** Back to how the demo looked when the page opened (it remembers what you did in this browser). */
  function reset() {
    try {
      for (const k of Object.keys(localStorage)) if (k.startsWith("demo:")) localStorage.removeItem(k);
    } catch { /* private window */ }
    ready = false;
    pending = [];
    active = 0;
    loads++;
    src = demoUrl();
  }

  onMount(() => {
    theme = (document.documentElement.dataset.theme as "dark" | "light") ?? "dark";
    // phones get the screenshots instead: the real interface needs a larger screen
    if (matchMedia("(min-width: 720px)").matches) src = demoUrl();

    const onMessage = (e: MessageEvent) => {
      if (e.origin !== location.origin || e.source !== frame?.contentWindow) return;
      if (e.data?.source === "genlauncher-demo" && e.data.type === "ready") {
        ready = true;
        for (const c of pending.splice(0)) post(c);
      }
    };
    window.addEventListener("message", onMessage);

    // the site's theme button also switches the demo
    const watch = new MutationObserver(() => {
      const t = document.documentElement.dataset.theme === "light" ? "light" : "dark";
      if (t !== theme) { theme = t; post({ type: "theme", value: t }); }
    });
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { window.removeEventListener("message", onMessage); watch.disconnect(); };
  });
</script>

<section class="wrap try" id="try" aria-labelledby="try-h">
  <p class="live"><span class="pulse" aria-hidden="true"></span> Live demo</p>
  <h2 id="try-h">Try it, right here</h2>
  <p class="sub">This is the real launcher interface, running in your browser with sample data. Click around: nothing is installed.</p>

  <div class="interactive">
    <div class="pills" role="group" aria-label="Things to try">
      {#each chapters as c, i}
        <button class:on={active === i} aria-pressed={active === i} onclick={() => choose(i)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={c.icon} /></svg>
          {c.title}
        </button>
      {/each}
    </div>
    <p class="caption" aria-live="polite">{chapters[active].text}</p>

    <div class="window">
      <div class="chrome">
        <i aria-hidden="true"></i><i aria-hidden="true"></i><i aria-hidden="true"></i>
        <span>GenLauncher</span>
        <div class="tools">
          <button onclick={reset} title="Start the demo over">Reset</button>
          <a href="/demo/" target="_blank" rel="noopener">Full screen</a>
        </div>
      </div>
      <div class="screen" bind:clientWidth={width} style:--s={scale} style:height="{H * scale}px">
        <img class="poster dark" src="/screens/library-dark.jpg" alt="" width="1360" height="850" />
        <img class="poster light" src="/screens/library-light.jpg" alt="" width="1360" height="850" />
        {#if src}
          <iframe bind:this={frame} {src} title="GenLauncher, interactive demo" class:ready></iframe>
        {/if}
      </div>
    </div>
    <p class="hint">Tip: the sun / moon button at the top of this page switches the demo's theme too.</p>
  </div>

  <div class="static">
    <img class="shot dark" src="/screens/library-dark.jpg" alt="The Library screen of GenLauncher" width="1360" height="850" loading="lazy" />
    <img class="shot light" src="/screens/library-light.jpg" alt="The Library screen of GenLauncher" width="1360" height="850" loading="lazy" />
    <p class="hint">The interactive demo needs a larger screen. <a href="/demo/">Open it anyway</a>.</p>
  </div>
</section>

<style>
  .try { text-align: center; }
  .live { display: inline-flex; align-items: center; gap: .55rem; margin: 0 0 .9rem; padding: .3rem .85rem; border-radius: 999px; border: 1px solid var(--stroke-hi);
    background: var(--surface); color: var(--text-dim); font-size: .85rem; font-weight: 600; letter-spacing: .02em; }
  .pulse { width: .55rem; height: .55rem; border-radius: 50%; background: #35d07f; box-shadow: 0 0 0 0 rgb(53 208 127 / .6); animation: pulse 2s infinite; }
  @keyframes pulse { 70% { box-shadow: 0 0 0 .55rem rgb(53 208 127 / 0); } 100% { box-shadow: 0 0 0 0 rgb(53 208 127 / 0); } }
  h2 { font-size: clamp(1.7rem, 3.4vw, 2.3rem); letter-spacing: -.02em; line-height: 1.15; margin: 0 0 .6rem; }
  .sub { color: var(--text-dim); margin: 0 auto 1.6rem; font-size: 1.08rem; max-width: 40rem; }

  .pills { display: flex; flex-wrap: wrap; justify-content: center; gap: .45rem; }
  .pills button { display: inline-flex; align-items: center; gap: .45rem; font: inherit; font-weight: 600; font-size: .93rem; color: var(--text-dim); background: var(--surface);
    border: 1px solid var(--stroke); padding: .5rem 1rem; border-radius: 999px; cursor: pointer; transition: background .2s, color .2s, border-color .2s, transform .2s var(--ease); }
  .pills button:hover { color: var(--text); border-color: var(--stroke-hi); transform: translateY(-1px); }
  .pills button.on { background: linear-gradient(135deg, var(--accent), var(--accent-2)); color: var(--accent-ink); border-color: transparent; }
  .caption { margin: 1.1rem auto 1.2rem; max-width: 44rem; color: var(--text-dim); min-height: 3.2rem; }

  .window { border-radius: 18px; border: 1px solid var(--stroke-hi); background: var(--surface-solid); box-shadow: var(--shadow), 0 0 90px var(--glow-1); overflow: hidden; text-align: left; }
  .chrome { display: flex; align-items: center; gap: .45rem; padding: .55rem .9rem; border-bottom: 1px solid var(--stroke); background: var(--bg-2); }
  .chrome i { width: .7rem; height: .7rem; border-radius: 50%; background: var(--stroke-hi); }
  .chrome span { position: absolute; inset-inline: 0; margin: 0 auto; width: max-content; font-size: .8rem; color: var(--text-faint); pointer-events: none; }
  .chrome { position: relative; }
  .tools { margin-inline-start: auto; display: flex; gap: .9rem; position: relative; z-index: 1; }
  .tools button, .tools a { font: inherit; font-size: .8rem; color: var(--text-dim); background: none; border: 0; padding: 0; cursor: pointer; text-decoration: none; }
  .tools button:hover, .tools a:hover { color: var(--accent); }

  .screen { position: relative; width: 100%; background: var(--bg-2); }
  .poster { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  iframe { position: absolute; top: 0; left: 0; width: 1240px; height: 780px; border: 0; transform: scale(var(--s)); transform-origin: top left; opacity: 0; transition: opacity .35s; background: var(--bg); }
  iframe.ready { opacity: 1; }
  :global(:root[data-theme="light"]) .dark, :global(:root:not([data-theme="light"])) .light { display: none; }
  .hint { margin: 1rem 0 0; color: var(--text-faint); font-size: .9rem; }

  .static { display: none; }
  .static .shot { width: 100%; height: auto; border-radius: 14px; border: 1px solid var(--stroke-hi); }
  @media (max-width: 719px) {
    .interactive { display: none; }
    .static { display: block; }
  }
</style>
