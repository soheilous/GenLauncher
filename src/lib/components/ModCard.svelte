<script lang="ts">
  import { app } from "$lib/app.svelte";
  import type { Item } from "$lib/types";
  import CoverArt from "./CoverArt.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { modName } from "$lib/names";

  let { mod, discover = false }: { mod: Item; discover?: boolean } = $props();
  const installed = $derived(mod.versions.some((v) => v.installed));
  const latestInstalled = $derived(mod.versions.find((v) => v.v === mod.latest)?.installed ?? false);
  const active = $derived(!discover && app.activeModId === mod.id);

  function activate() {
    if (discover) app.install(mod);
    else app.selectMod(mod.id);
  }
</script>

<div class="card" class:active role="button" tabindex="0" onclick={activate}
  onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); activate(); } }}>
  <div class="art">
    <CoverArt item={mod} />
    {#if mod.progress !== undefined}
      <div class="prog"><i style:width="{mod.progress * 100}%"></i></div>
    {/if}
    {#if installed && !latestInstalled && !discover}
      <span class="chip warn tag">{t("Update {version}", { version: mod.latest })}</span>
    {:else if installed && !discover}
      <span class="chip ok tag"><Icon name="check" size={12} /> {t("Installed")}</span>
    {/if}
  </div>
  <div class="meta">
    <strong><bdi>{modName(mod)}</bdi></strong>
    <small><bdi>{discover ? `${mod.latest}${mod.sizeMb ? " · " + (mod.sizeMb / 1000).toFixed(1) + " GB" : ""}` : (mod.versions.find((v) => v.installed)?.v ?? mod.latest)}</bdi></small>
  </div>
  {#if discover}
    <span class="btn primary get" title={mod.direct ? t("Direct download: published by the author, installs with one click") : undefined}>{#if mod.direct && mod.progress === undefined}<Icon name="bolt" size={12} />{/if}{mod.progress !== undefined ? (mod.progress ? Math.round(mod.progress * 100) + "%" : "…") : t("Install")}</span>
  {/if}
</div>

<style>
  .card { position: relative; border-radius: var(--r-md); background: var(--surface); border: 1px solid var(--stroke);
    backdrop-filter: var(--blur); overflow: hidden; cursor: pointer; transition: transform .25s var(--ease), border-color .2s, box-shadow .25s; }
  .card:hover { transform: translateY(-3px); border-color: var(--stroke-hi); box-shadow: var(--shadow); }
  .card.active { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent), 0 12px 36px hsl(var(--accent-h) 80% 50% / .28); }
  .card:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .art { height: 118px; position: relative; }
  .tag { position: absolute; top: 10px; inset-inline-start: 10px; backdrop-filter: blur(10px); z-index: 1; }
  .prog { position: absolute; left: 0; right: 0; bottom: 0; height: 5px; background: rgb(0 0 0 / .35); }
  .prog i { display: block; height: 100%; background: var(--accent); transition: width .14s linear; }
  .meta { padding: 12px 14px 14px; display: flex; flex-direction: column; gap: 2px; }
  .meta small { color: var(--text-faint); }
  .get { position: absolute; inset-inline-end: 12px; bottom: 12px; padding: 6px 14px; font-size: 12px; }
</style>
