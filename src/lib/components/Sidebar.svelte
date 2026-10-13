<script lang="ts">
  import { app, type View } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";

  const nav: { id: View; label: string; icon: string }[] = [
    { id: "library", label: "Library", icon: "library" },
    { id: "discover", label: "Discover", icon: "discover" },
    { id: "downloads", label: "Downloads", icon: "download" },
    { id: "notifications", label: "Notifications", icon: "bell" },
  ];
  const collapsed = $derived(app.sidebarCollapsed);
  import { isTauri } from "$lib/backend";
  import type { Game } from "$lib/types";
  const canUse = (g: Game) => !isTauri || !!app.installFor(g);
  const gameLabel = $derived(app.game === "zh" ? "Zero Hour" : "Generals");
</script>

<aside class="glass side" class:collapsed>
  <div class="brand">
    <div class="logo">G</div>
    <div class="name"><strong>GenLauncher</strong><small>2.0 preview</small></div>
    <button class="collapse" onclick={() => app.toggleSidebar()} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      aria-expanded={!collapsed} title="{collapsed ? 'Expand' : 'Collapse'} sidebar (Ctrl+B)">
      <Icon name={collapsed ? "expand" : "collapse"} size={16} />
    </button>
  </div>

  {#if collapsed}
    <button class="game-mini" title="{gameLabel} (click to switch)" aria-label="Switch game, current {gameLabel}"
      onclick={() => app.switchGame(app.game === "zh" ? "generals" : "zh")}>{app.game === "zh" ? "ZH" : "G"}</button>
  {:else}
    <div class="games" role="tablist" aria-label="Game">
      <button role="tab" aria-selected={app.game === "zh"} class:on={app.game === "zh"} disabled={!canUse("zh")} title={canUse("zh") ? undefined : "Zero Hour was not found"} onclick={() => app.switchGame("zh")}>Zero Hour</button>
      <button role="tab" aria-selected={app.game === "generals"} class:on={app.game === "generals"} disabled={!canUse("generals")} title={canUse("generals") ? undefined : "Generals was not found"} onclick={() => app.switchGame("generals")}>Generals</button>
    </div>
  {/if}

  <nav>
    {#each nav as n}
      <button class="nav-item" class:on={app.view === n.id} onclick={() => (app.view = n.id)} title={collapsed ? n.label : undefined} aria-label={n.label}>
        <Icon name={n.icon} /> <span class="label">{n.label}</span>
        {#if n.id === "downloads" && app.downloading.length}<span class="badge">{app.downloading.length}</span>{/if}
        {#if n.id === "notifications" && app.unread}<span class="badge" aria-label="{app.unread} unread">{app.unread}</span>{/if}
      </button>
    {/each}
  </nav>

  <div class="foot">
    <button class="nav-item" onclick={() => (app.settingsOpen = true)} title={collapsed ? "Settings" : undefined} aria-label="Settings">
      <Icon name="settings" /> <span class="label">Settings</span>
    </button>
    <button class="nav-item" onclick={() => app.toggleTheme()} aria-label="Toggle theme" title={collapsed ? "Toggle theme" : undefined}>
      <Icon name={app.theme === "dark" ? "moon" : "sun"} /> <span class="label">{app.theme === "dark" ? "Dark" : "Light"} theme</span>
    </button>
  </div>
</aside>

<style>
  .side { width: 220px; padding: 18px 12px; display: flex; flex-direction: column; gap: 16px; flex: none; overflow: hidden;
    transition: width .28s var(--ease), padding .28s var(--ease); }
  .side.collapsed { width: 72px; padding-inline: 10px; }

  .brand { display: flex; gap: 10px; align-items: center; padding: 0 4px; min-height: 36px; position: relative; }
  .name { flex: 1; min-width: 0; white-space: nowrap; }
  .name small { display: block; color: var(--text-faint); font-size: 11px; }
  .logo { width: 36px; height: 36px; border-radius: 12px; display: grid; place-items: center; font-weight: 800; color: var(--accent-ink); flex: none;
    background: linear-gradient(135deg, var(--accent), hsl(calc(var(--accent-h) + 50) 80% 55%)); }
  .collapse { width: 28px; height: 28px; border-radius: 50%; display: grid; place-items: center; flex: none; color: var(--text-dim);
    background: var(--surface); border: 1px solid var(--stroke); transition: background .15s, color .15s; }
  .collapse:hover { background: var(--surface-hi); color: var(--text); }
  .collapse:focus-visible, .game-mini:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .collapsed .brand { flex-direction: column; padding: 0; gap: 10px; }
  .collapsed .name { display: none; }

  .games { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; padding: 4px; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); }
  .games button { padding: 7px 0; border-radius: 999px; font-size: 12px; font-weight: 600; color: var(--text-dim); transition: background .2s, color .2s; white-space: nowrap; }
  .games button:disabled { opacity: .4; cursor: not-allowed; }
  .games button.on { background: var(--accent); color: var(--accent-ink); }
  .game-mini { align-self: center; width: 40px; height: 40px; border-radius: 14px; font-size: 12px; font-weight: 700; background: var(--accent); color: var(--accent-ink); }

  nav, .foot { display: flex; flex-direction: column; gap: 4px; }
  nav { flex: 1; }
  .nav-item { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 14px; color: var(--text-dim); text-align: left; position: relative; white-space: nowrap; transition: background .18s, color .18s; }
  .nav-item:hover { background: var(--surface-hi); color: var(--text); }
  .nav-item.on { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .nav-item :global(svg) { flex: none; }
  .badge { margin-left: auto; background: var(--accent); color: var(--accent-ink); border-radius: 999px; font-size: 11px; font-weight: 700; padding: 1px 7px; }

  .collapsed .label { display: none; }
  .collapsed .nav-item { justify-content: center; padding: 11px 0; }
  .collapsed .badge { position: absolute; top: 2px; right: 4px; margin: 0; padding: 0 5px; font-size: 10px; }
</style>
