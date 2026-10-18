<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { isTauri, mapsPreview } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import { formatBytes } from "$lib/guess";
  import { tgaToDataUrl } from "$lib/tga";
  import { i18n, t, tn } from "$lib/i18n/index.svelte";

  const hueOf = (s: string) => [...s].reduce((h, c) => (h * 33 + c.charCodeAt(0)) % 360, 7);
  let query = $state("");
  $effect(() => { void app.refreshMaps(); });
  const shown = $derived(app.maps.filter((m) => m.name.toLowerCase().includes(query.trim().toLowerCase())));

  // preview pictures are read when a map comes into view (a hundred maps are not all read at once)
  let previews = $state<Record<string, string | null>>({});
  const asked = new Set<string>();
  async function load(name: string) {
    if (asked.has(name)) return;
    asked.add(name);
    if (!isTauri) { previews[name] = null; return; }
    try { previews[name] = tgaToDataUrl(new Uint8Array(await mapsPreview(name))); }
    catch { previews[name] = null; }
  }
  /** Svelte action: ask for the preview once the element is on screen. */
  function whenVisible(node: HTMLElement, name: string) {
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) { void load(name); io.disconnect(); }
    }, { rootMargin: "200px" });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
</script>

{#snippet picture(name: string)}
  <div class="pic" use:whenVisible={name} style:--h={hueOf(name)}>
    {#if previews[name]}<img src={previews[name]} alt="" />{:else}<span aria-hidden="true">{name.slice(0, 1)}</span>{/if}
  </div>
{/snippet}

<div class="maps">
  <div class="bar">
    <label class="search glass">
      <Icon name="search" size={16} />
      <input placeholder={t("Search maps")} bind:value={query} aria-label={t("Search maps")} />
    </label>
    <div class="seg" role="group" aria-label={t("How the maps are shown")}>
      <button class:on={app.mapsView === "cards"} aria-pressed={app.mapsView === "cards"} onclick={() => app.setMapsView("cards")}><Icon name="image" size={14} /> {t("Cards")}</button>
      <button class:on={app.mapsView === "list"} aria-pressed={app.mapsView === "list"} onclick={() => app.setMapsView("list")}><Icon name="library" size={14} /> {t("List")}</button>
    </div>
    <button class="btn primary" disabled={app.mapsBusy} onclick={() => app.addMaps(false)}><Icon name="plus" size={16} /> {t("Add maps from a file…")}</button>
    <button class="btn" disabled={app.mapsBusy} onclick={() => app.addMaps(true)}><Icon name="folder" size={16} /> {t("From a folder…")}</button>
    {#if isTauri}<button class="btn" onclick={() => app.openMapsFolder()}><Icon name="shortcut" size={16} /> {t("Open the Maps folder")}</button>{/if}
  </div>
  <p class="hint">{t("Custom maps live in your Documents, in the game's data folder. Add a map pack (a zip, 7z or rar, or a folder, or drop it here) and each map is put where the game looks for it.")}</p>

  <p class="count" aria-live="polite">{tn(shown.length, "{n} map", "{n} maps")}</p>
  {#if app.mapsView === "cards"}
    <div class="cards">
      {#each shown as m (m.name)}
        <div class="glass card">
          {@render picture(m.name)}
          <div class="meta">
            <strong title={m.name}><bdi>{m.name}</bdi></strong>
            <small>{formatBytes(m.bytes)}{m.modified ? " · " + i18n.date(m.modified * 1000) : ""}</small>
          </div>
          <button class="btn del" aria-label={t("Remove {name}", { name: m.name })} title={t("Remove {name}", { name: m.name })} disabled={app.mapsBusy} onclick={() => app.removeMap(m.name)}><Icon name="trash" size={14} /></button>
        </div>
      {:else}
        <p class="empty">{app.mapsBusy ? t("Looking at your maps…") : t("No custom maps yet.")}</p>
      {/each}
    </div>
  {:else}
    <div class="list">
      {#each shown as m (m.name)}
        <div class="glass row">
          {@render picture(m.name)}
          <div class="who"><strong><bdi>{m.name}</bdi></strong><small>{formatBytes(m.bytes)}{m.modified ? " · " + i18n.date(m.modified * 1000) : ""}</small></div>
          <button class="btn" aria-label={t("Remove {name}", { name: m.name })} title={t("Remove {name}", { name: m.name })} disabled={app.mapsBusy} onclick={() => app.removeMap(m.name)}><Icon name="trash" size={14} /></button>
        </div>
      {:else}
        <p class="empty">{app.mapsBusy ? t("Looking at your maps…") : t("No custom maps yet.")}</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .maps { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; overflow-y: auto; padding: 4px 6px 12px 4px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .search { display: flex; align-items: center; gap: 10px; padding: 9px 16px; border-radius: 999px; color: var(--text-dim); box-shadow: none; flex: 1 1 220px; }
  .search input { background: none; border: 0; outline: none; flex: 1; min-width: 0; user-select: text; }
  .seg { display: flex; padding: 3px; gap: 3px; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); }
  .seg button { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-radius: 999px; font-size: 13px; color: var(--text-dim); }
  .seg button.on { background: var(--accent); color: var(--accent-ink); font-weight: 600; }
  .hint, .count { margin: 0; color: var(--text-faint); font-size: 13px; }

  .pic { flex: none; aspect-ratio: 1; display: grid; place-items: center; overflow: hidden; background: linear-gradient(135deg, hsl(var(--h) 45% 30%), hsl(calc(var(--h) + 40) 55% 16%)); }
  .pic img { width: 100%; height: 100%; object-fit: cover; image-rendering: auto; }
  .pic span { font-size: 1.6rem; font-weight: 800; color: rgb(255 255 255 / .8); }

  .list { display: grid; gap: 8px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 8px 12px; border-radius: var(--r-md); }
  .row .pic { width: 54px; border-radius: 10px; }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who small { color: var(--text-faint); }

  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(168px, 1fr)); gap: 14px; }
  .card { position: relative; border-radius: var(--r-md); overflow: hidden; display: flex; flex-direction: column; }
  .card .pic { width: 100%; }
  .meta { padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .meta strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta small { color: var(--text-faint); }
  .del { position: absolute; top: 8px; inset-inline-end: 8px; padding: 6px; opacity: 0; transition: opacity .15s; background: rgb(0 0 0 / .45); color: #fff; }
  .card:hover .del, .del:focus-visible { opacity: 1; }
  .empty { color: var(--text-faint); text-align: center; padding: 40px 0; grid-column: 1 / -1; }
</style>
