<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";
  import Toggle from "./Toggle.svelte";
  import Select from "./Select.svelte";
  import { N, i18n, offered, t, tn } from "$lib/i18n/index.svelte";

  let dialog: HTMLDivElement | undefined = $state();
  let withCovers = $state(true);
  $effect(() => { if (app.settingsOpen) { dialog?.focus(); void app.refreshBackups(); void app.refreshCloudBackups(); } });

  const LABELS: Record<string, string> = { manual: N("Backup"), "before-import": N("Before an import"), "before-restore": N("Before a restore"), auto: N("Automatic") };
  const languageOptions = $derived([{ value: "auto", label: t("Automatic (system language)") }, ...offered().map((l) => ({ value: l.code, label: l.name }))]);
  const when = (at: number) => i18n.dateTime(at * 1000);
</script>

<svelte:window onkeydown={(e) => { if (app.settingsOpen && e.key === "Escape") app.settingsOpen = false; }} />

{#if app.settingsOpen}
  <div class="scrim" role="presentation" onclick={() => (app.settingsOpen = false)}></div>
  <div class="glass dlg" role="dialog" aria-modal="true" aria-label={t("Settings")} tabindex="-1" bind:this={dialog}>
    <header>
      <h2>{t("Settings")}</h2>
      <button class="btn" aria-label={t("Close")} onclick={() => (app.settingsOpen = false)}><Icon name="x" size={16} /></button>
    </header>

    <div class="grid">
      <h3>{t("Appearance")}</h3>
      <div class="line"><span>{t("Language")}</span>
        <Select label={t("Language")} options={languageOptions} value={i18n.setting} onchange={(v) => i18n.choose(v)} /></div>
      <div class="line"><span>{t("Theme")}</span>
        <div class="seg">
          <button class:on={app.theme === "dark"} onclick={() => app.setTheme("dark")}><Icon name="moon" size={14} /> {t("Dark")}</button>
          <button class:on={app.theme === "light"} onclick={() => app.setTheme("light")}><Icon name="sun" size={14} /> {t("Light")}</button>
        </div>
      </div>
      <div class="line"><span>{t("Use mod cover as window background")}</span>
        <Toggle label={t("Cover as background")} checked={app.coverBackdrop} onchange={(v) => app.setPref("coverBackdrop", v)} /></div>
      <div class="line"><span>{t("Accent follows selected mod")}</span>
        <Toggle label={t("Accent follows mod")} checked={app.accentFromMod} onchange={(v) => app.setPref("accentFromMod", v)} /></div>

      <h3>{t("Game")}</h3>
      <div class="line"><span>{t("Camera height")}</span>
        <div class="slider">
          <input type="range" min="0" max="500" step="10" value={app.cameraHeight}
            aria-label={t("Camera height")} oninput={(e) => app.setPref("cameraHeight", +e.currentTarget.value)} />
          <output>{app.cameraHeight === 0 ? t("Default") : app.cameraHeight}</output>
        </div>
      </div>
      <div class="line"><span>{t("Run windowed")}</span><Toggle label={t("Windowed")} checked={app.windowed} onchange={(v) => app.setPref("windowed", v)} /></div>
      <div class="line"><span>{t("Skip intro (quick start)")}</span><Toggle label={t("Quick start")} checked={app.quickStart} onchange={(v) => app.setPref("quickStart", v)} /></div>
      <div class="line"><span>{t("Resolution, graphics, extra arguments")}</span>
        <button class="btn" onclick={() => { app.settingsOpen = false; app.optionsOpen = true; }}><Icon name="settings" size={14} /> {t("Game options…")}</button></div>
    </div>
    <h3>{t("Backup")}</h3>
    <div class="line"><span>{t("Include my custom covers")}</span><Toggle label={t("Include custom covers")} checked={withCovers} onchange={(v) => (withCovers = v)} /></div>
    <div class="line">
      <span>{t("Settings of both games and the look of the launcher")}</span>
      <div class="pair">
        <button class="btn" onclick={() => app.exportConfiguration(withCovers)}><Icon name="download" size={14} /> {t("Export…")}</button>
        <button class="btn" onclick={() => app.importConfiguration()}><Icon name="folder" size={14} /> {t("Import…")}</button>
      </div>
    </div>
    <div class="line"><span>{t("Versioned backups (the newest 20 are kept)")}</span>
      <button class="btn" onclick={() => app.backUpNow(withCovers)}><Icon name="plus" size={14} /> {t("Back up now")}</button></div>
    {#each app.backups.slice(0, 6) as b (b.name)}
      <div class="line bk"><span>{when(b.at)} <small>{t(LABELS[b.label] ?? b.label)}</small></span>
        <div class="pair">
          <button class="btn" onclick={() => app.restoreBackup(b)}><Icon name="undo" size={14} /> {t("Restore")}</button>
          <button class="btn" aria-label={t("Delete this backup")} title={t("Delete this backup")} onclick={() => app.deleteBackup(b)}><Icon name="trash" size={14} /></button>
        </div></div>
    {/each}
    {#if app.backups.length > 6}<p class="note">{tn(app.backups.length - 6, "{n} older backup is kept too.", "{n} older backups are kept too.")}</p>{/if}
    <h3>{t("Google Drive")}</h3>
    {#if app.google}
      <div class="line"><span>{t("Signed in as {account}", { account: app.google.email || t("your Google account") })}</span>
        <button class="btn" onclick={() => app.signOutGoogle()}>{t("Sign out")}</button></div>
      <div class="line"><span>{t("Back up automatically when settings change")}</span>
        <Toggle label={t("Back up automatically")} checked={app.autoCloud} onchange={(v) => app.setAutoCloud(v)} /></div>
      <div class="line"><span>{t("Saved in a hidden folder only GenLauncher can see")}</span>
        <button class="btn" onclick={() => app.cloudBackUpNow("manual", withCovers)}><Icon name="plus" size={14} /> {t("Back up now")}</button></div>
      {#each app.cloudBackups.slice(0, 6) as b (b.id)}
        <div class="line bk"><span>{when(b.at)} <small>{t(LABELS[b.label] ?? b.label)}</small></span>
          <div class="pair">
            <button class="btn" onclick={() => app.restoreCloudBackup(b)}><Icon name="undo" size={14} /> {t("Restore")}</button>
            <button class="btn" aria-label={t("Delete this backup")} title={t("Delete this backup")} onclick={() => app.deleteCloudBackup(b)}><Icon name="trash" size={14} /></button>
          </div></div>
      {/each}
    {:else}
      <div class="line"><span>{t("Keep your settings in your own Google Drive and bring them to a new PC")}</span>
        <button class="btn primary" disabled={app.googleBusy} onclick={() => app.signInGoogle()}>{app.googleBusy ? t("Waiting for the browser…") : t("Sign in with Google")}</button></div>
    {/if}
    <h3>{t("Updates")}</h3>
    <div class="line"><span>{app.update ? t("Version {version} is available.", { version: app.update.version }) : t("GenLauncher {version}", { version: app.appVersion })}</span>
      {#if app.update}
        <button class="btn primary" disabled={app.updateBusy || app.launch !== "idle"} onclick={() => app.installUpdate()}>{app.updateBusy ? t("Updating…") : t("Update and restart")}</button>
      {:else}
        <button class="btn" disabled={app.updateBusy} onclick={() => app.checkUpdate(true)}>{app.updateBusy ? t("Checking…") : t("Check for updates")}</button>
      {/if}</div>
    <p class="note">{t("A configuration file holds your choices only (active mod, enabled patches, options, theme). Mod files are never included.")}</p>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: var(--scrim); backdrop-filter: blur(4px); z-index: 20; animation: fade .2s; }
  .dlg { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px));
    padding: 22px; background: var(--surface-solid); animation: pop .25s var(--ease); outline: none; max-height: calc(100vh - 32px); overflow: auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  h2 { font-size: 20px; }
  h3 { font-size: 11px; text-transform: uppercase; letter-spacing: .08em; color: var(--text-faint); margin: 18px 0 4px; }
  .line { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 0; border-bottom: 1px solid var(--stroke); }
  .seg { display: flex; padding: 3px; gap: 3px; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); }
  .seg button { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-radius: 999px; font-size: 13px; color: var(--text-dim); }
  .seg button.on { background: var(--accent); color: var(--accent-ink); font-weight: 600; }
  .pair { display: flex; gap: 8px; }
  .bk small { margin-inline-start: 8px; color: var(--text-faint); }
  .slider { display: flex; align-items: center; gap: 12px; }
  input[type="range"] { accent-color: var(--accent); width: 170px; }
  output { min-width: 56px; text-align: end; color: var(--text-dim); font-variant-numeric: tabular-nums; }
  .note { margin-top: 16px; color: var(--text-faint); font-size: 12px; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -46%) scale(.97); } }
</style>
