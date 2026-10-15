<script lang="ts">
  import { GAMES, type SiteMod } from "$lib/data";
  import { getI18n } from "$lib/i18n";

  let { mod }: { mod: SiteMod } = $props();
  let broken = $state(false);
  const extras = $derived(mod.patches.length + mod.addons.length);
  const { t, tn } = getI18n();
</script>

<article class="card" class:old={mod.deprecated}>
  <div class="art" style:--h={mod.hue}>
    {#if mod.image && !broken}
      <img src={mod.image} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (broken = true)} />
    {:else}
      <span class="initial" aria-hidden="true">{mod.name.slice(0, 1)}</span>
    {/if}
  </div>
  <div class="body">
    <h3>{mod.name}</h3>
    <p class="meta">
      <span class="tag">{GAMES[mod.game]}</span>
      {#if mod.version}<span>{mod.version}</span>{/if}
      {#if extras}<span>{tn(mod.patches.length, "{n} patch", "{n} patches")} · {tn(mod.addons.length, "{n} add-on", "{n} add-ons")}</span>{/if}
    </p>
    <div class="links">
      {#if mod.page}<a href={mod.page} rel="noopener noreferrer" target="_blank">{t("Mod page")}</a>{/if}
      {#if mod.discord}<a href={mod.discord} rel="noopener noreferrer" target="_blank">Discord</a>{/if}
      {#if mod.deprecated}<span class="warn">{t("No longer maintained")}</span>{/if}
    </div>
  </div>
</article>

<style>
  .card { display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); overflow: hidden;
    transition: transform .25s var(--ease), border-color .25s, box-shadow .25s; }
  .card:hover { transform: translateY(-3px); border-color: var(--stroke-hi); box-shadow: var(--shadow); }
  .card.old { opacity: .72; }
  .art { aspect-ratio: 5 / 2; display: grid; place-items: center; overflow: hidden;
    background: linear-gradient(135deg, hsl(var(--h) 60% 32%), hsl(calc(var(--h) + 40) 70% 18%)); }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .initial { font-size: 2.4rem; font-weight: 800; color: rgb(255 255 255 / .85); }
  .body { padding: .9rem 1rem 1.05rem; display: flex; flex-direction: column; gap: .35rem; flex: 1; }
  h3 { margin: 0; font-size: 1.05rem; }
  .meta { margin: 0; display: flex; flex-wrap: wrap; gap: .25rem .7rem; color: var(--text-faint); font-size: .85rem; }
  .tag { color: var(--accent); font-weight: 600; }
  .links { margin-top: auto; padding-top: .5rem; display: flex; flex-wrap: wrap; gap: .3rem 1rem; font-size: .9rem; }
  .warn { color: var(--text-faint); font-style: italic; }
</style>
