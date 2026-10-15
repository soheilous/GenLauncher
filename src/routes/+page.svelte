<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app } from "$lib/app.svelte";
  import { art, coverStyle } from "$lib/art";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import ModCard from "$lib/components/ModCard.svelte";
  import DetailPanel from "$lib/components/DetailPanel.svelte";
  import LaunchBar from "$lib/components/LaunchBar.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import AddDialog from "$lib/components/AddDialog.svelte";
  import OptionsDialog from "$lib/components/OptionsDialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { formatBytes } from "$lib/guess";

  onMount(() => app.init());

  /** Entries that were unread when the page was opened stay highlighted while it is open. */
  let fresh = $state(new Set<string>());
  $effect(() => {
    if (app.view !== "notifications") return;
    untrack(() => {
      fresh = new Set(app.notices.filter((n) => !n.read).map((n) => n.id));
      app.markAllRead();
    });
  });

  function ago(at: number): string {
    const s = Math.max(0, Math.round((Date.now() - at) / 1000));
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.floor(s / 60)} min ago`;
    if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
    return new Date(at).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
  }
  const levelIcon = { info: "bell", success: "check", error: "x", news: "discover" } as const;

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") { e.preventDefault(); app.toggleSidebar(); }
  }
</script>

<svelte:window {onkeydown} />

<div class="stage">
  <!-- One layer per mod so switching mods crossfades the backdrop. -->
  {#each app.mods as m (m.id)}
    <div class="backdrop" class:on={m.id === app.activeMod.id}
      style:background={app.coverBackdrop ? coverStyle(m) : art(m.hue)}></div>
  {/each}
  <div class="veil"></div>

  <div class="shell">
    <Sidebar />

    <main>
      <header class="top">
        <div>
          <h1>{app.view === "library" ? "Library" : app.view === "discover" ? "Discover" : app.view === "notifications" ? "Notifications" : "Downloads"}</h1>
          <p>
            {#if app.view === "notifications"}
              {app.unread ? `${app.unread} unread` : "Everything you were told, newest first"}
            {:else}
            {app.game === "zh" ? "Zero Hour" : "Generals"} · {app.installedMods.length} installed
            {#if app.phase === "ready" && !app.online}
              <span class="chip warn offline" title={app.catalogError ?? ""}>Offline · showing saved data</span>
            {/if}
            {/if}
          </p>
        </div>
        {#if app.phase === "ready" && (app.view === "library" || app.view === "discover")}
          <div class="tools">
            <label class="search glass">
              <Icon name="search" size={16} />
              <input placeholder="Search mods" bind:value={app.search} aria-label="Search mods" />
            </label>
            <button class="btn primary" onclick={() => app.openAdd("mod")}><Icon name="plus" size={16} /> Add mod</button>
          </div>
        {:else if app.phase === "ready" && app.view === "notifications"}
          <div class="tools">
            <button class="btn" disabled={!app.unread} onclick={() => app.markAllRead()}><Icon name="check" size={16} /> Mark all read</button>
            <button class="btn" disabled={!app.notices.length} onclick={() => app.clearNotices()}><Icon name="trash" size={16} /> Clear all</button>
          </div>
        {/if}
      </header>

      {#if app.phase === "setup"}
        <div class="center">
          <div class="glass setup">
            <img class="big-logo" src="/logo.svg" alt="" width="56" height="56" />
            <h2>Find your game</h2>
            <p>Select the folder that contains <b>generals.exe</b>: your Command &amp; Conquer Generals or Zero Hour install. GenLauncher keeps everything it needs in a <code>.genlauncher</code> folder inside it and never changes your original game files permanently.</p>
            {#each app.installs as i}
              <button class="btn" onclick={() => app.openInstall(i.dir)}>{i.game === "zh" ? "Zero Hour" : "Generals"} · {i.dir}</button>
            {/each}
            <button class="btn primary" onclick={() => app.chooseFolder()}><Icon name="folder" size={16} /> Browse for folder…</button>
          </div>
        </div>
      {:else if app.phase === "loading"}
        <div class="scroll" aria-busy="true" aria-label="Loading library">
          <div class="grid">
            {#each Array(8) as _, i}<div class="skeleton glass" style:animation-delay="{i * 90}ms"></div>{/each}
          </div>
        </div>
      {:else if app.view === "library"}
        <div class="split">
          <div class="scroll">
            <div class="grid">
              {#each app.installedMods as m (m.id)}<ModCard mod={m} />{/each}
            </div>
            {#if app.installedMods.length === 0}<p class="empty">No installed mods match your search.</p>{/if}
          </div>
          <DetailPanel />
        </div>
        <LaunchBar />
      {:else if app.view === "discover"}
        <div class="scroll">
          <div class="grid">
            {#each app.availableMods as m (m.id)}<ModCard mod={m} discover />{/each}
          </div>
          {#if app.availableMods.length === 0}<p class="empty">Everything available is already installed.</p>{/if}
        </div>
      {:else if app.view === "notifications"}
        <div class="scroll">
          {#each app.notices as n (n.id)}
            <div class="glass note {n.level}" class:fresh={fresh.has(n.id)}>
              <span class="ico"><Icon name={levelIcon[n.level]} size={16} /></span>
              <div class="body">
                {#if n.title}<strong>{n.title}</strong>{/if}
                <p>{n.text}</p>
                <small>{n.level === "news" ? "From GenLauncher · " : ""}{ago(n.at)}</small>
              </div>
              {#if n.link}<button class="btn" onclick={() => app.openPage(n.link ?? "")}><Icon name="link" size={14} /> Open</button>{/if}
              <button class="btn" aria-label="Dismiss" title="Dismiss" onclick={() => app.removeNotice(n.id)}><Icon name="x" size={14} /></button>
            </div>
          {:else}
            <p class="empty">Nothing yet. Install results, game sessions and messages from GenLauncher will show up here.</p>
          {/each}
        </div>
      {:else}
        <div class="scroll">
          {#each app.downloading as d (d.id)}
            <div class="glass dl">
              <div class="dlinfo"><strong>{d.name}</strong><small>{app.versionLabel(d)} · {app.phaseLabel(d)}{d.bytes ? " · " + formatBytes(d.bytes) + " received" : ""}</small>
                <div class="bar"><i style:width="{(d.progress ?? 0) * 100}%"></i></div></div>
              <span>{Math.round((d.progress ?? 0) * 100)}%</span>
              <button class="btn" aria-label="Cancel download" onclick={() => app.cancel(d)}><Icon name="x" size={14} /></button>
            </div>
          {:else}
            <p class="empty">No active downloads. Install something from Discover.</p>
          {/each}
        </div>
      {/if}
    </main>
  </div>

  <SettingsDialog />
  <AddDialog />
  {#if app.dragging}<div class="dropveil" aria-hidden="true"><div class="glass"><Icon name="download" size={28} /><strong>Drop to add a mod</strong><span>zip, 7z, rar, a .big file or a folder</span></div></div>{/if}
  <OptionsDialog />
  {#if app.toast}<div class="toast glass" role="status">{app.toast}</div>{/if}
</div>

<style>
  .stage { position: fixed; inset: 0; overflow: hidden; }
  .backdrop { position: absolute; inset: -10%; filter: blur(46px) saturate(1.25); opacity: 0; transition: opacity .6s; }
  .backdrop.on { opacity: .8; }
  .veil { position: absolute; inset: 0; background: linear-gradient(180deg, var(--scrim), var(--bg) 120%); }
  .shell { position: relative; display: flex; gap: 16px; padding: 16px; height: 100%; }
  main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 16px; }
  .top { display: flex; justify-content: space-between; align-items: center; padding: 4px 6px 0; }
  h1 { font-size: 28px; font-weight: 700; letter-spacing: -.01em; }
  .top p { color: var(--text-dim); }
  .tools { display: flex; align-items: center; gap: 12px; }
  .search { display: flex; align-items: center; gap: 10px; padding: 9px 16px; border-radius: 999px; color: var(--text-dim); box-shadow: none; }
  .search input { background: none; border: 0; outline: none; width: 220px; user-select: text; }
  .split { flex: 1; display: flex; gap: 16px; min-height: 0; }
  .scroll { flex: 1; overflow-y: auto; min-height: 0; padding: 4px 6px 12px 4px; display: flex; flex-direction: column; gap: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 16px; }
  .empty { color: var(--text-faint); text-align: center; padding: 40px 0; }
  .offline { margin-left: 8px; }
  .center { flex: 1; display: grid; place-items: center; }
  .setup { width: min(520px, 100%); padding: 32px; display: flex; flex-direction: column; gap: 14px; align-items: stretch; text-align: center; background: var(--surface-solid); }
  .setup p { color: var(--text-dim); }
  .setup code { background: var(--surface-hi); padding: 1px 6px; border-radius: 6px; }
  .setup .btn { justify-content: center; word-break: break-all; }
  .big-logo { width: 56px; height: 56px; border-radius: 18px; margin: 0 auto; }
  .skeleton { height: 190px; border-radius: var(--r-md); animation: pulse 1.4s ease-in-out infinite; box-shadow: none; }
  @keyframes pulse { 50% { opacity: .45; } }
  .dl { display: flex; align-items: center; gap: 16px; padding: 16px 20px; border-radius: var(--r-md); }
  .dlinfo { flex: 1; display: flex; flex-direction: column; gap: 4px; }
  .dlinfo small { color: var(--text-faint); }
  .bar { height: 6px; border-radius: 6px; background: var(--stroke); overflow: hidden; margin-top: 6px; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width .14s linear; }
  .note { display: flex; align-items: flex-start; gap: 14px; padding: 14px 18px; border-radius: var(--r-md); }
  .note.fresh { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent-soft); }
  .note .ico { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; flex: none; background: var(--surface-hi); color: var(--text-dim); }
  .note.success .ico { color: var(--ok, #35d07f); }
  .note.error .ico { color: var(--danger); }
  .note.news .ico { color: var(--accent); }
  .note .body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .note .body p { overflow-wrap: anywhere; user-select: text; }
  .note small { color: var(--text-faint); }
  .dropveil { position: fixed; inset: 0; z-index: 40; display: grid; place-items: center; background: var(--scrim); backdrop-filter: blur(6px); pointer-events: none; }
  .dropveil .glass { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 34px 54px; border: 2px dashed var(--accent); background: var(--surface-solid); color: var(--text); border-radius: var(--r-lg); }
  .dropveil span { color: var(--text-dim); }
  .toast { position: fixed; bottom: 108px; left: 50%; transform: translateX(-50%); padding: 10px 20px; border-radius: 999px; z-index: 30; animation: rise .3s var(--ease); background: var(--surface-solid); }
  @keyframes rise { from { opacity: 0; transform: translate(-50%, 10px); } }
  @media (max-width: 1000px) { :global(.detail) { display: none; } }
</style>
