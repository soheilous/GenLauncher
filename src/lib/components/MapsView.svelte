<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { isTauri } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import { formatBytes } from "$lib/guess";
  import { i18n, t, tn } from "$lib/i18n/index.svelte";

  let query = $state("");
  $effect(() => { void app.refreshMaps(); });
  const shown = $derived(app.maps.filter((m) => m.name.toLowerCase().includes(query.trim().toLowerCase())));
</script>

<div class="maps">
  <div class="bar">
    <label class="search glass">
      <Icon name="search" size={16} />
      <input placeholder={t("Search maps")} bind:value={query} aria-label={t("Search maps")} />
    </label>
    <button class="btn primary" disabled={app.mapsBusy} onclick={() => app.addMaps(false)}><Icon name="plus" size={16} /> {t("Add maps from a file…")}</button>
    <button class="btn" disabled={app.mapsBusy} onclick={() => app.addMaps(true)}><Icon name="folder" size={16} /> {t("From a folder…")}</button>
    {#if isTauri}<button class="btn" onclick={() => app.openMapsFolder()}><Icon name="shortcut" size={16} /> {t("Open the Maps folder")}</button>{/if}
  </div>
  <p class="hint">{t("Custom maps live in your Documents, in the game's data folder. Add a map pack (a zip, 7z or rar, or a folder, or drop it here) and each map is put where the game looks for it.")}</p>

  <p class="count" aria-live="polite">{tn(shown.length, "{n} map", "{n} maps")}</p>
  <div class="list">
    {#each shown as m (m.name)}
      <div class="glass row">
        <div class="who"><strong><bdi>{m.name}</bdi></strong><small>{formatBytes(m.bytes)}{m.modified ? " · " + i18n.date(m.modified * 1000) : ""}</small></div>
        <button class="btn" aria-label={t("Remove {name}", { name: m.name })} title={t("Remove {name}", { name: m.name })} disabled={app.mapsBusy} onclick={() => app.removeMap(m.name)}><Icon name="trash" size={14} /></button>
      </div>
    {:else}
      <p class="empty">{app.mapsBusy ? t("Looking at your maps…") : t("No custom maps yet.")}</p>
    {/each}
  </div>
</div>

<style>
  .maps { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; overflow-y: auto; padding: 4px 6px 12px 4px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .search { display: flex; align-items: center; gap: 10px; padding: 9px 16px; border-radius: 999px; color: var(--text-dim); box-shadow: none; flex: 1 1 220px; }
  .search input { background: none; border: 0; outline: none; flex: 1; min-width: 0; user-select: text; }
  .hint, .count { margin: 0; color: var(--text-faint); font-size: 13px; }
  .list { display: grid; gap: 8px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 10px 16px; border-radius: var(--r-md); }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who small { color: var(--text-faint); }
  .empty { color: var(--text-faint); text-align: center; padding: 40px 0; }
</style>
