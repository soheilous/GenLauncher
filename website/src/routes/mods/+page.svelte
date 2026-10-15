<script lang="ts">
  import ModCard from "$lib/ModCard.svelte";
  import { CATALOG, GAMES, mods, stats, type GameKey } from "$lib/data";

  let query = $state("");
  let game = $state<"all" | GameKey>("all");
  let sort = $state<"name" | "extras">("name");
  let onlyExtras = $state(false);

  const shown = $derived(
    mods
      .filter((m) => (game === "all" || m.game === game) && (!onlyExtras || m.patches.length + m.addons.length > 0))
      .filter((m) => {
        const q = query.trim().toLowerCase();
        return !q || [m.name, ...m.patches, ...m.addons].some((s) => s.toLowerCase().includes(q));
      })
      .toSorted((a, b) => (sort === "extras" ? b.patches.length + b.addons.length - (a.patches.length + a.addons.length) || a.name.localeCompare(b.name) : a.name.localeCompare(b.name)))
  );
  const GAME_TABS: { id: "all" | GameKey; label: string }[] = [{ id: "all", label: "All" }, { id: "zh", label: GAMES.zh }, { id: "generals", label: GAMES.generals }];
</script>

<svelte:head>
  <title>Mods · GenLauncher</title>
  <meta name="description" content="The GenLauncher mod catalog: {stats.mods} Generals and Zero Hour mods with their patches and add-ons, each linking to its author's own page." />
</svelte:head>

<div class="wrap page">
  <h1>Mod catalog</h1>
  <p class="lead">{stats.mods} mods with {stats.patches} patches and {stats.addons} add-ons. This list only points to each mod's own page; every download comes from its author. Last updated {stats.updated}.</p>

  <div class="tools" role="search">
    <label class="search">
      <span class="sr-only">Search mods, patches and add-ons</span>
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><path d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm9 16-4.2-4.2"/></svg>
      <input type="search" placeholder="Search mods, patches and add-ons" bind:value={query} />
    </label>
    <div class="seg" role="group" aria-label="Game">
      {#each GAME_TABS as t}<button class:on={game === t.id} aria-pressed={game === t.id} onclick={() => (game = t.id)}>{t.label}</button>{/each}
    </div>
    <label class="check"><input type="checkbox" bind:checked={onlyExtras} /> With patches or add-ons</label>
    <label class="sort">Sort
      <select bind:value={sort}><option value="name">A to Z</option><option value="extras">Most add-ons</option></select>
    </label>
  </div>

  <p class="count" aria-live="polite">{shown.length} {shown.length === 1 ? "mod" : "mods"}</p>
  <div class="grid">
    {#each shown as m (m.game + m.name)}<ModCard mod={m} />{/each}
  </div>
  {#if shown.length === 0}<p class="empty">Nothing matches. Try another word, or <a href="{CATALOG}" rel="noopener">add the mod to the index</a>.</p>{/if}

  <p class="note">Missing a mod, a patch, or a newer version? The catalog is a plain file in its own repository: <a href="{CATALOG}" rel="noopener">send a pull request</a>.</p>
</div>

<style>
  .page { padding-top: 3rem; }
  h1 { font-size: clamp(2rem, 4.5vw, 3rem); letter-spacing: -.025em; margin: 0 0 .6rem; }
  .lead { color: var(--text-dim); max-width: 44rem; margin: 0 0 1.8rem; font-size: 1.08rem; }
  .tools { display: flex; flex-wrap: wrap; align-items: center; gap: .8rem 1.1rem; margin-bottom: 1rem; }
  .search { display: flex; align-items: center; gap: .6rem; flex: 1 1 18rem; padding: .6rem 1rem; border-radius: 999px; border: 1px solid var(--stroke-hi); background: var(--surface); color: var(--text-dim); }
  .search:focus-within { border-color: var(--accent); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: none; color: var(--text); font: inherit; }
  .seg { display: inline-flex; gap: .2rem; padding: .25rem; border-radius: 999px; border: 1px solid var(--stroke); background: var(--surface); }
  .seg button { font: inherit; font-weight: 600; font-size: .93rem; color: var(--text-dim); background: none; border: 0; padding: .4rem .95rem; border-radius: 999px; cursor: pointer; }
  .seg button.on { background: var(--accent); color: var(--accent-ink); }
  .check { color: var(--text-dim); display: inline-flex; align-items: center; gap: .5rem; cursor: pointer; }
  .check input { accent-color: var(--accent); width: 1rem; height: 1rem; }
  .sort { color: var(--text-dim); display: inline-flex; align-items: center; gap: .5rem; margin-left: auto; }
  select { font: inherit; color: var(--text); background: var(--surface-solid); border: 1px solid var(--stroke-hi); border-radius: 10px; padding: .4rem .6rem; }
  .count { color: var(--text-faint); font-size: .92rem; margin: 0 0 1rem; }
  .grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fill, minmax(16.5rem, 1fr)); }
  .empty, .note { color: var(--text-faint); margin-top: 1.5rem; }
</style>
