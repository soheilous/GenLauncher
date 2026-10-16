<script lang="ts">
  import { app } from "$lib/app.svelte";
  import CoverArt from "./CoverArt.svelte";
  import { formatBytes } from "$lib/guess";
  import { processCover } from "$lib/image";
  import type { ModKind } from "$lib/types";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import { N, t } from "$lib/i18n/index.svelte";
  import { modName } from "$lib/names";

  const tabs: { kind: ModKind; label: string }[] = [
    { kind: "patch", label: N("Patches") },
    { kind: "addon", label: N("Addons") },
    { kind: "exe", label: N("Executables") },
  ];
  let tab = $state<ModKind>("patch");
  const mod = $derived(app.activeMod);
  const defaultVersion = $derived(mod.versions.find((v) => v.installed)?.v ?? mod.latest);
  const versionOptions = $derived(
    mod.versions.map((v) => ({
      value: v.v,
      label: v.v,
      hint: !v.installed ? t("Not installed") : v.v === mod.latest ? t("Latest") : t("Installed"),
      muted: !v.installed,
    }))
  );
  const list = $derived(app.children.filter((c) => c.kind === tab));

  const selectedVersion = $derived(app.chosenVersions[mod.id] ?? defaultVersion);
  const selectedInstalled = $derived(mod.versions.some((v) => v.v === selectedVersion && v.installed));
  const latestInstalled = $derived(mod.versions.some((v) => v.v === mod.latest && v.installed));
  const anyInstalled = $derived(mod.versions.some((v) => v.installed));
  const canInstall = $derived(!!mod.latest && !latestInstalled && mod.id !== "original-game");
  let renaming = $state(false);
  let draft = $state("");
  function startRename() { draft = selectedVersion; renaming = true; }
  async function finishRename() {
    const ok = await app.renameVersion(mod, selectedVersion, draft);
    if (ok) renaming = false;
  }
  // another mod or version was picked: stop editing
  $effect(() => { void mod.id; void selectedVersion; renaming = false; });
  const canRemove = $derived(selectedInstalled && mod.id !== "original-game");
  /** Each version remembers the page it was downloaded from (it can differ per version and per site);
   *  without one, fall back to the mod's own page from the catalog. */
  const versionPage = $derived(mod.versions.find((v) => v.v === selectedVersion)?.page || "");
  const pageUrl = $derived(versionPage || mod.links?.moddb || "");
  const pageLabel = $derived(versionPage ? t("Open the page this version was downloaded from") : t("Open the mod's web page"));

  const PHASES: Record<string, string> = {
    preparing: N("Preparing…"), downloading: N("Downloading"), verifying: N("Checking files…"), extracting: N("Extracting…"), finishing: N("Finishing…"),
  };
  /** "Downloading 42%" while the size is known, otherwise just the step. */
  function status(it: { progress?: number; phase?: string; bytes?: number }) {
    const label = t(PHASES[it.phase ?? ""] ?? N("Working…"));
    if (it.phase === "downloading" && it.progress) return `${label} ${Math.round(it.progress * 100)}%`;
    return it.bytes ? `${label} · ${formatBytes(it.bytes)}` : label;
  }
  function hasUpdate(it: { versions: { v: string; installed: boolean }[]; latest: string }) {
    return it.versions.some((v) => v.installed) && !it.versions.some((v) => v.v === it.latest && v.installed);
  }

  let fileInput: HTMLInputElement | undefined = $state();
  let dragOver = $state(false);
  const hasCustom = $derived(mod.id in app.customCovers);

  async function applyCover(file: Blob | undefined | null) {
    if (!file) return;
    try {
      const { dataUrl, hue } = await processCover(file);
      app.setCover(mod.id, dataUrl, hue);
      app.say(t("Cover updated for {name}", { name: mod.name }));
    } catch (e) {
      app.say(e instanceof Error ? e.message : t("Could not load that image"));
    }
  }
</script>

<section class="glass detail">
  <div class="hero" class:drag={dragOver} role="presentation"
    ondragover={(e) => { e.preventDefault(); dragOver = true; }}
    ondragleave={() => (dragOver = false)}
    ondrop={(e) => { e.preventDefault(); dragOver = false; applyCover(e.dataTransfer?.files[0]); }}>
    <CoverArt item={mod} pad={34} />
    <div class="fade"></div>
    <h2><bdi>{modName(mod)}</bdi></h2>
    <div class="cover-actions">
      {#if hasCustom}
        <button class="mini" title={t("Reset cover")} aria-label={t("Reset cover")} onclick={() => app.resetCover(mod.id)}><Icon name="undo" size={15} /></button>
      {/if}
      <button class="mini" title={t("Change cover (or drop an image here)")} aria-label={t("Change cover")} onclick={() => fileInput?.click()}>
        <Icon name="image" size={15} /> <span>{t("Change cover")}</span>
      </button>
    </div>
    <input bind:this={fileInput} type="file" accept="image/*" hidden
      onchange={(e) => { applyCover(e.currentTarget.files?.[0]); e.currentTarget.value = ""; }} />
    {#if dragOver}<div class="dropnote">{t("Drop image to set as cover")}</div>{/if}
  </div>

  <div class="body">
    {#if mod.summary}<p class="sum">{mod.summary}</p>{/if}

    <div class="row2">
      <span class="lbl">{t("Version")}</span>
      {#if renaming}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="rename" aria-label={t("New name for this version")} bind:value={draft} autofocus
          onkeydown={(e) => { if (e.key === "Enter") void finishRename(); else if (e.key === "Escape") renaming = false; }} />
        <button class="btn icon" title={t("Save the name")} aria-label={t("Save the name")} onclick={() => finishRename()}><Icon name="check" size={15} /></button>
        <button class="btn icon" title={t("Cancel")} aria-label={t("Cancel")} onclick={() => (renaming = false)}><Icon name="x" size={15} /></button>
      {:else}
        <Select label={t("Version")} options={versionOptions} value={selectedVersion} onchange={(v) => app.setChosenVersion(mod.id, v)} />
      {/if}
      {#if pageUrl}
        <button class="btn icon" title={pageLabel} aria-label={pageLabel} onclick={() => app.openPage(pageUrl)}><Icon name="link" size={15} /></button>
      {/if}
      {#if versionPage && selectedInstalled}
        <button class="btn icon" title={t("Check for a newer version (opens the page this version came from)")} aria-label={t("Check for a newer version")}
          onclick={() => app.checkForUpdate(mod, versionPage)}><Icon name="refresh" size={15} /></button>
      {/if}
      <button class="btn icon" title={anyInstalled || mod.id === "original-game" ? t("Show the installed files") : t("Not installed yet")}
        aria-label={t("Show the installed files")} disabled={!anyInstalled && mod.id !== "original-game"}
        onclick={() => app.openFolder(mod, selectedVersion)}><Icon name="folder" size={15} /></button>
      {#if canRemove && !renaming}
        <button class="btn icon" title={t("Rename this version")} aria-label={t("Rename this version")} disabled={mod.progress !== undefined} onclick={startRename}><Icon name="edit" size={15} /></button>
      {/if}
      {#if canRemove}
        <button class="btn icon danger" title={t("Remove version {version}", { version: selectedVersion })} aria-label={t("Remove version {version}", { version: selectedVersion })}
          disabled={mod.progress !== undefined} onclick={() => app.uninstall(mod, selectedVersion)}><Icon name="trash" size={15} /></button>
      {/if}
    </div>

    {#if mod.progress !== undefined}
      <div class="install">
        <div class="meter" class:unknown={!mod.progress}><i style:width="{Math.max(mod.progress, 0.02) * 100}%"></i></div>
        <div class="install-row"><span>{app.pending[mod.id] ? t("Adding {version}", { version: app.pending[mod.id].version }) + " · " : ""}{status(mod)}</span>
          <button class="btn" onclick={() => app.cancel(mod)}><Icon name="x" size={13} /> {t("Cancel")}</button></div>
      </div>
    {:else if canInstall}
      <button class="btn primary wide" onclick={() => app.install(mod)}>
        <Icon name="download" size={16} /> {anyInstalled ? t("Update to {version}", { version: mod.latest }) : t("Install {version}", { version: mod.latest })}
      </button>
    {/if}

    <div class="tabs" role="tablist">
      {#each tabs as tb}
        <button role="tab" aria-selected={tab === tb.kind} class:on={tab === tb.kind} onclick={() => (tab = tb.kind)}>
          {t(tb.label)}<span>{app.children.filter((c) => c.kind === tb.kind && c.selected).length || ""}</span>
        </button>
      {/each}
    </div>

    <div class="list">
      {#each list as it (it.id)}
        {@const has = it.versions.some((v) => v.installed)}
        <div class="row" role="button" tabindex="0" onclick={() => app.toggleChild(it)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); app.toggleChild(it); } }}>
          <span class="box" class:checked={it.selected}>{#if it.selected}<Icon name="check" size={13} />{/if}</span>
          <div class="txt">
            <strong><bdi>{it.name}</bdi></strong>
            <small>{it.progress !== undefined ? status(it) : has ? t("Installed {version}", { version: it.versions.find((v) => v.installed)?.v ?? "" }) : t("Not installed")}</small>
            {#if it.progress !== undefined}<div class="bar"><i style:width="{Math.max(it.progress, 0.02) * 100}%"></i></div>{/if}
          </div>
          {#if it.progress !== undefined}
            <button class="chip warn" aria-label={t("Cancel installing {name}", { name: it.name })} onclick={(e) => { e.stopPropagation(); app.cancel(it); }}>{t("Cancel")}</button>
          {:else if has}
            <button class="chip trash" title={t("Remove {version}", { version: it.versions.find((v) => v.installed)?.v ?? "" })} aria-label={t("Remove {name}", { name: it.name })}
              onclick={(e) => { e.stopPropagation(); app.uninstall(it, it.versions.find((v) => v.installed)?.v ?? ""); }}><Icon name="trash" size={14} /></button>
            {#if hasUpdate(it)}<button class="chip warn" title={t("Update to {version}", { version: it.latest })} aria-label={t("Update {name} to {version}", { name: it.name, version: it.latest })} onclick={(e) => { e.stopPropagation(); app.install(it); }}>{t("Update")}</button>
            {:else}<span class="chip"><bdi>{it.latest}</bdi></span>{/if}
          {:else}
            <span class="chip accent">{t("Get")}</span>
          {/if}
        </div>
      {:else}
        <p class="empty">{app.loadingExtras ? t("Loading…") : t("Nothing here for this mod.")}</p>
      {/each}
      <button class="add-extra" onclick={() => app.openAdd(tab, mod.id)}>
        <Icon name="plus" size={14} /> {tab === "exe" ? t("Add an executable from a file or link") : tab === "addon" ? t("Add an add-on from a file or link") : t("Add a patch from a file or link")}
      </button>
    </div>
  </div>
</section>

<style>
  .detail { width: 360px; flex: none; display: flex; flex-direction: column; overflow: hidden; }
  .hero { height: 170px; position: relative; display: flex; align-items: flex-end; padding: 16px 20px; }
  .fade { position: absolute; inset: 0; background: linear-gradient(to top, rgb(0 0 0 / .72), transparent 55%); }
  .hero.drag { outline: 2px dashed var(--accent); outline-offset: -8px; }
  .cover-actions { z-index: 2; position: absolute; top: 12px; inset-inline-end: 12px; display: flex; gap: 6px; opacity: 0; transition: opacity .2s; }
  .hero:hover .cover-actions, .hero:focus-within .cover-actions { opacity: 1; }
  .mini { display: inline-flex; align-items: center; gap: 6px; padding: 6px 11px; border-radius: 999px; font-size: 12px; font-weight: 600; color: #fff;
    background: rgb(0 0 0 / .45); backdrop-filter: blur(10px); border: 1px solid rgb(255 255 255 / .25); transition: background .15s; }
  .mini:hover { background: rgb(0 0 0 / .65); }
  .mini:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .dropnote { position: absolute; inset: 0; display: grid; place-items: center; background: rgb(0 0 0 / .5); color: #fff; font-weight: 600; }
  h2 { position: relative; font-size: 24px; font-weight: 700; color: #fff; text-shadow: 0 2px 12px rgb(0 0 0 / .45); }
  .body { padding: 16px 20px 20px; display: flex; flex-direction: column; gap: 14px; min-height: 0; flex: 1; }
  .sum { color: var(--text-dim); }
  .row2 { display: flex; gap: 8px; align-items: center; }
  .row2 .lbl { color: var(--text-faint); font-size: 12px; }
  .icon { padding: 8px; }
  .icon.danger:hover { color: var(--danger); border-color: var(--danger); }
  .wide { width: 100%; justify-content: center; }
  .install { display: flex; flex-direction: column; gap: 8px; }
  .install-row { display: flex; justify-content: space-between; align-items: center; color: var(--text-dim); font-size: 13px; }
  .install-row .btn { padding: 5px 12px; font-size: 12px; }
  .meter { height: 8px; border-radius: 8px; background: var(--stroke); overflow: hidden; }
  .meter i { display: block; height: 100%; background: var(--accent); transition: width .2s linear; }
  .meter.unknown i { width: 35% !important; animation: slide 1.1s ease-in-out infinite alternate; }
  @keyframes slide { from { transform: translateX(0); } to { transform: translateX(185%); } }
  button.chip { cursor: pointer; }
  .rename { flex: 1; min-width: 0; height: 36px; padding: 0 12px; border-radius: 999px; border: 1px solid var(--accent); background: var(--surface); color: var(--text); font: inherit; outline: none; }
  .chip.trash { padding: 6px; display: inline-grid; place-items: center; }
  .chip.trash:hover { color: var(--danger); border-color: var(--danger); }
  .row:has(.trash) { gap: 8px; }
  .add-extra { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 10px; border-radius: 14px; color: var(--text-dim); font-size: 13px;
    border: 1px dashed var(--stroke-hi); transition: color .15s, border-color .15s, background .15s; }
  .add-extra:hover { color: var(--accent); border-color: var(--accent); background: var(--accent-soft); }
  .add-extra:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tabs { display: flex; gap: 4px; padding: 4px; background: var(--surface); border: 1px solid var(--stroke); border-radius: 999px; }
  .tabs button { flex: 1; padding: 6px 4px; border-radius: 999px; font-size: 12px; font-weight: 600; color: var(--text-dim); transition: background .2s, color .2s; }
  .tabs button.on { background: var(--surface-hi); color: var(--text); }
  .tabs span { margin-inline-start: 5px; color: var(--accent); }
  .list { display: flex; flex-direction: column; gap: 6px; overflow-y: auto; min-height: 0; padding-inline-end: 2px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 14px; background: var(--surface); border: 1px solid var(--stroke); cursor: pointer; transition: background .15s, border-color .15s; }
  .row:hover { background: var(--surface-hi); border-color: var(--stroke-hi); }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .box { width: 20px; height: 20px; border-radius: 7px; border: 1.5px solid var(--stroke-hi); display: grid; place-items: center; flex: none; color: var(--accent-ink); transition: background .15s; }
  .box.checked { background: var(--accent); border-color: var(--accent); }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .txt small { color: var(--text-faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .bar { height: 4px; border-radius: 4px; background: var(--stroke); margin-top: 6px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width .14s linear; }
  .empty { color: var(--text-faint); padding: 20px 0; text-align: center; }
</style>
