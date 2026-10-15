<script lang="ts">
  import Faq from "$lib/Faq.svelte";
  import Rich from "$lib/Rich.svelte";
  import { REPO } from "$lib/data";
  import { N, getI18n } from "$lib/i18n";

  const { t } = getI18n();

  const toc = [
    ["start", N("Getting started")],
    ["mods", N("Adding mods")],
    ["play", N("Playing")],
    ["settings", N("Your settings")],
    ["problems", N("When something goes wrong")],
    ["faq", N("Questions")],
  ];
</script>

<svelte:head>
  <title>{t("Guide · GenLauncher")}</title>
  <meta name="description" content={t("How to install mods, switch patches and add-ons, back up your settings and fix common problems with GenLauncher.")} />
</svelte:head>

<div class="wrap page">
  <div class="prose">
    <h1>{t("Guide")}</h1>
    <p>{t("Everything you need to get from \"I have the game\" to \"I'm playing a mod\".")}</p>
    <nav class="toc" aria-label={t("On this page")}>{#each toc as [id, label]}<a href="#{id}">{t(label)}</a>{/each}</nav>

    <h2 id="start">{t("Getting started")}</h2>
    <p><Rich text={t("Start GenLauncher. It looks for Zero Hour and Generals (Steam, EA App and disc installs) and opens the one it finds. If it cannot find your game, choose the folder that contains `generals.exe`. Everything the launcher keeps goes into a `.genlauncher` folder inside your game folder.")} /></p>
    <p>{t("If you used the old GenLauncher, a one-time offer moves your existing mods into the new layout and removes the old launcher's files. Mods are moved, never deleted.")}</p>

    <h2 id="mods">{t("Adding mods")}</h2>
    <p><Rich text={t("**From Discover.** Open *Discover*, pick a mod and press *Install*. A window opens on the mod's own page. Choose the version you want and click the page's download button. The window tucks itself away as soon as the download starts, you confirm the name and version, and the mod installs by itself.")} /></p>
    <p><Rich text={t("**Add mod.** Press *Add mod* to use a zip, 7z or rar you downloaded yourself, an already unpacked folder, a direct download link, or any web page. Patches, add-ons and executables are added from the tabs of the mod they belong to.")} /></p>
    <p><Rich text={t("**Several versions.** A mod with the same name becomes another version of it, and the launcher warns you first so two different mods are never mixed up. Each version remembers its page: the link button opens it, and the refresh button reopens it so you can fetch a newer release.")} /></p>
    <p><Rich text={t("Program installers (`.exe`, `.msi`) are refused: mods are archives and folders.")} /></p>

    <h2 id="play">{t("Playing")}</h2>
    <p><Rich text={t("Select a mod in the *Library*, choose its version, switch on the patches, add-ons and executables you want (one patch and one executable at a time) and press *Play*. Windowed mode and quick start are one click away in the bar below.")} /></p>
    <p><Rich text={t("*Settings → Game options* edits the game's own settings: resolution, texture quality, particle limit and effects. The original `Options.ini` is kept once as `Options.ini.bak`.")} /></p>
    <p>{t("While the game runs the launcher changes your game folder; when the game closes everything is restored. If the PC crashed in between, the next start finishes that cleanup before anything else.")}</p>

    <h2 id="settings">{t("Your settings")}</h2>
    <p><Rich text={t("*Settings → Backup* exports your settings to a file, imports one, and keeps versioned backups on this PC (the newest 20). Importing or restoring always backs up your current settings first, so you can undo it.")} /></p>
    <p><Rich text={t("You can also sign in with Google to keep the same backups in a hidden folder of your own Google Drive. It is optional; see the [privacy policy](/privacy).")} /></p>

    <h2 id="problems">{t("When something goes wrong")}</h2>
    <ul>
      <li><Rich text={t("**Windows says \"protected your PC\".** The installer is not code-signed yet. Click *More info*, then *Run anyway*.")} /></li>
      <li><Rich text={t("**The Notifications page** keeps the message of every failed install, in full.")} /></li>
      <li><Rich text={t("**A download stops moving.** After a minute the launcher cancels it and takes you back to the link step so you can try again.")} /></li>
      <li><Rich text={t("**Mod folders out of place?** Use the folder button next to a version to open it in the file manager.")} /></li>
      <li><Rich text={t("**Still stuck?** [Open an issue]({url}) and say which mod and version, and what the Notifications page says.", { url: REPO + "/issues" })} /></li>
    </ul>

    <h2 id="faq">{t("Questions")}</h2>
  </div>
  <Faq />
</div>

<style>
  .page { padding-top: 3rem; }
  .toc { display: flex; flex-wrap: wrap; gap: .5rem; margin: 1.2rem 0 .5rem; }
  .toc a { padding: .35rem .9rem; border-radius: 999px; border: 1px solid var(--stroke); background: var(--surface); color: var(--text-dim); text-decoration: none; font-size: .93rem; }
  .toc a:hover { color: var(--text); border-color: var(--stroke-hi); }
  .prose :global(h2) { scroll-margin-top: 5.5rem; }
</style>
