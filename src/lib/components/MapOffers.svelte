<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";
  import { formatBytes } from "$lib/guess";
  import { t, tn } from "$lib/i18n/index.svelte";

  $effect(() => { void app.refreshMapOffers(); });
  const hueOf = (s: string) => [...s].reduce((h, c) => (h * 33 + c.charCodeAt(0)) % 360, 7);
  const jobOf = (name: string) => app.mapJobList.find((j) => j.name === name);
</script>

<div class="offers">
  <p class="hint">{t("Map packs the authors publish. Where the author offers a direct download, GenLauncher fetches it and puts every map where the game looks. Otherwise the author's page opens: download the file there and add it on the Maps page (or drop it there).")}</p>
  <p class="count" aria-live="polite">{tn(app.shownMapOffers.length, "{n} map pack", "{n} map packs")}</p>
  <div class="cards">
    {#each app.shownMapOffers as m (m.name)}
      {@const job = jobOf(m.name)}
      <div class="glass card">
        <div class="pic" style:--h={hueOf(m.name)}>
          {#if m.image}<img src={m.image} alt="" loading="lazy" />{:else}<span aria-hidden="true">{m.name.slice(0, 1)}</span>{/if}
        </div>
        <div class="meta">
          <strong title={m.name}><bdi>{m.name}</bdi></strong>
          {#if m.author}<small><bdi>{m.author}</bdi></small>{/if}
          {#if m.description}<p>{m.description}</p>{/if}
        </div>
        <div class="act">
          {#if job}
            <div class="bar" class:unknown={!job.p?.total}><i style:width="{Math.max(job.p?.total ? job.p.done / job.p.total : 0, 0.02) * 100}%"></i></div>
            <button class="btn" aria-label={t("Stop this download")} title={t("Stop this download")} onclick={() => app.cancelMapJob(job.id)}><Icon name="x" size={14} /> {t("Stop")}</button>
          {:else}
            {#if m.direct}
              <button class="btn primary" onclick={() => app.installMapPack(m)}><Icon name="bolt" size={14} /> {t("Install")}{m.direct.size ? " · " + formatBytes(m.direct.size) : ""}</button>
            {/if}
            {#if m.page}
              <button class="btn" onclick={() => app.openPage(m.page ?? "")}><Icon name="link" size={14} /> {t("Open page")}</button>
            {/if}
          {/if}
        </div>
      </div>
    {:else}
      <p class="empty">{t("No map packs match your search.")}</p>
    {/each}
  </div>
</div>

<style>
  .offers { display: flex; flex-direction: column; gap: 10px; }
  .hint, .count { margin: 0; color: var(--text-faint); font-size: 13px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 14px; }
  .card { border-radius: var(--r-md); overflow: hidden; display: flex; flex-direction: column; }
  .pic { aspect-ratio: 16 / 9; display: grid; place-items: center; overflow: hidden; background: linear-gradient(135deg, hsl(var(--h) 45% 30%), hsl(calc(var(--h) + 40) 55% 16%)); }
  .pic img { width: 100%; height: 100%; object-fit: cover; }
  .pic span { font-size: 2rem; font-weight: 800; color: rgb(255 255 255 / .8); }
  .meta { padding: 10px 12px 4px; display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1; }
  .meta strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta small { color: var(--text-faint); }
  .meta p { margin: 4px 0 0; font-size: 13px; color: var(--text-dim); display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .act { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px 12px 12px; }
  .bar { flex: 1; height: 6px; border-radius: 3px; background: var(--surface-hi); overflow: hidden; min-width: 60px; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width .2s; }
  .bar.unknown i { animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: .4; } }
  .empty { color: var(--text-faint); text-align: center; padding: 40px 0; grid-column: 1 / -1; }
</style>
