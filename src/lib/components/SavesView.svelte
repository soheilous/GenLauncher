<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { isTauri } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import Toggle from "./Toggle.svelte";
  import { formatBytes } from "$lib/guess";
  import { i18n, t, tn } from "$lib/i18n/index.svelte";

  let { kind }: { kind: "save" | "replay" } = $props();
  $effect(() => { void app.refreshProfiles(); });
  const files = $derived(app.profileFileList.filter((f) => f.kind === kind));
  const when = (secs: number) => (secs ? i18n.date(secs * 1000) : "");
  const targets = $derived(app.profileTargets().filter((k) => k !== app.profileSelected));
  const targetOptions = $derived([{ value: "", label: t("Move to…") }, ...targets.map((k) => ({ value: k, label: app.profileLabel(k) }))]);
</script>

<div class="saves">
  <div class="glass intro">
    <div class="text">
      <strong>{t("Keep each mod's saved games and replays apart")}</strong>
      <p>{t("The game puts every saved game and replay in one place, whatever mod made it, and they do not work with another mod. With this on, GenLauncher keeps a set for each mod version and, before starting the game, puts the set of the mod you start where the game looks. Nothing is copied or deleted: the other sets are moved aside and come back when you start that mod again. Games started outside GenLauncher use whatever set was put there last.")}</p>
    </div>
    <Toggle label={t("Keep each mod's saved games and replays apart")} checked={app.savesByMod} onchange={(v) => app.setSavesByMod(v)} />
  </div>

  {#if app.profiles.length > 1}
    <div class="merge">
      <span>{t("There are several sets. You can bring them all together into the selected one.")}</span>
      <button class="btn" onclick={() => app.mergeProfiles()}><Icon name="plus" size={14} /> {t("Merge all sets into the selected one")}</button>
    </div>
  {/if}

  <div class="cols">
    <div class="profiles">
      {#each app.profiles as p (p.key)}
        <button class="glass profile" class:on={p.key === app.profileSelected} onclick={() => app.selectProfile(p.key)}>
          <strong><bdi>{app.profileLabel(p.key)}</bdi></strong>
          <small>{kind === "save" ? tn(p.saves, "{n} saved game", "{n} saved games") : tn(p.replays, "{n} replay", "{n} replays")}</small>
          {#if p.active}<span class="chip ok">{t("In the game's folder now")}</span>{/if}
        </button>
      {:else}
        <p class="empty">{kind === "save" ? t("No saved games yet.") : t("No replays yet.")}</p>
      {/each}
    </div>

    <div class="files">
      {#if app.profileSelected}
        <div class="head">
          <h3><bdi>{app.profileLabel(app.profileSelected)}</bdi></h3>
          {#if isTauri}<button class="btn" onclick={() => app.openProfileFolder()}><Icon name="folder" size={14} /> {t("Open the folder")}</button>{/if}
        </div>
        {#each files as f (f.kind + f.name)}
          <div class="glass row">
            <span class="kind" title={f.kind === "save" ? t("Saved game") : t("Replay")}><Icon name={f.kind === "save" ? "save" : "play"} size={14} /></span>
            <div class="who"><strong><bdi>{f.name}</bdi></strong><small>{formatBytes(f.bytes)}{f.modified ? " · " + when(f.modified) : ""}</small></div>
            <div class="mv"><Select label={t("Move to…")} options={targetOptions} value="" onchange={(v) => app.moveSaveFile(f, v)} /></div>
            <button class="btn" aria-label={t("Delete {name}", { name: f.name })} title={t("Delete {name}", { name: f.name })} onclick={() => app.deleteSaveFile(f)}><Icon name="trash" size={14} /></button>
          </div>
        {:else}
          <p class="empty">{t("Nothing in this set.")}</p>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .saves { display: flex; flex-direction: column; gap: 14px; min-height: 0; flex: 1; overflow-y: auto; padding: 4px 6px 12px 4px; }
  .intro { display: flex; align-items: center; gap: 18px; padding: 16px 20px; border-radius: var(--r-md); }
  .intro .text { flex: 1; min-width: 0; }
  .intro p { margin: 6px 0 0; color: var(--text-dim); font-size: 13px; }
  .merge { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; color: var(--text-dim); font-size: 13px; }
  .cols { display: grid; grid-template-columns: minmax(220px, 300px) 1fr; gap: 14px; align-items: start; }
  .profiles { display: grid; gap: 8px; }
  .profile { display: flex; flex-direction: column; align-items: flex-start; gap: 3px; padding: 12px 14px; border-radius: var(--r-md); text-align: start; cursor: pointer; border: 1px solid var(--stroke); }
  .profile.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent-soft); }
  .profile small { color: var(--text-faint); }
  .files { display: grid; gap: 8px; min-width: 0; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .head h3 { margin: 0; font-size: 16px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 8px 12px; border-radius: var(--r-md); }
  .kind { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 50%; background: var(--surface-hi); color: var(--text-dim); flex: none; }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who small { color: var(--text-faint); }
  .mv { width: 220px; flex: none; display: flex; }
  .empty { color: var(--text-faint); padding: 24px 0; text-align: center; }
  @media (max-width: 1100px) { .cols { grid-template-columns: 1fr; } .mv { width: 160px; } }
</style>
