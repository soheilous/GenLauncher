<script lang="ts">
  import Faq from "$lib/Faq.svelte";
  import { REPO } from "$lib/data";

  const toc = [
    ["start", "Getting started"],
    ["mods", "Adding mods"],
    ["play", "Playing"],
    ["settings", "Your settings"],
    ["problems", "When something goes wrong"],
    ["faq", "Questions"],
  ];
</script>

<svelte:head>
  <title>Guide · GenLauncher</title>
  <meta name="description" content="How to install mods, switch patches and add-ons, back up your settings and fix common problems with GenLauncher." />
</svelte:head>

<div class="wrap page">
  <div class="prose">
    <h1>Guide</h1>
    <p>Everything you need to get from "I have the game" to "I'm playing a mod".</p>
    <nav class="toc" aria-label="On this page">{#each toc as [id, label]}<a href="#{id}">{label}</a>{/each}</nav>

    <h2 id="start">Getting started</h2>
    <p>Start GenLauncher. It looks for Zero Hour and Generals (Steam, EA App and disc installs) and opens the one it finds. If it cannot find your game, choose the folder that contains <code>generals.exe</code>. Everything the launcher keeps goes into a <code>.genlauncher</code> folder inside your game folder.</p>
    <p>If you used the old GenLauncher, a one-time offer moves your existing mods into the new layout and removes the old launcher's files. Mods are moved, never deleted.</p>

    <h2 id="mods">Adding mods</h2>
    <p><strong>From Discover.</strong> Open <em>Discover</em>, pick a mod and press <em>Install</em>. A window opens on the mod's own page. Choose the version you want and click the page's download button. The window tucks itself away as soon as the download starts, you confirm the name and version, and the mod installs by itself.</p>
    <p><strong>Add mod.</strong> Press <em>Add mod</em> to use a zip, 7z or rar you downloaded yourself, an already unpacked folder, a direct download link, or any web page. Patches, add-ons and executables are added from the tabs of the mod they belong to.</p>
    <p><strong>Several versions.</strong> A mod with the same name becomes another version of it, and the launcher warns you first so two different mods are never mixed up. Each version remembers its page: the link button opens it, and the refresh button reopens it so you can fetch a newer release.</p>
    <p>Program installers (<code>.exe</code>, <code>.msi</code>) are refused: mods are archives and folders.</p>

    <h2 id="play">Playing</h2>
    <p>Select a mod in the <em>Library</em>, choose its version, switch on the patches, add-ons and executables you want (one patch and one executable at a time) and press <em>Play</em>. Windowed mode and quick start are one click away in the bar below.</p>
    <p><em>Settings → Game options</em> edits the game's own settings: resolution, texture quality, particle limit and effects. The original <code>Options.ini</code> is kept once as <code>Options.ini.bak</code>.</p>
    <p>While the game runs the launcher changes your game folder; when the game closes everything is restored. If the PC crashed in between, the next start finishes that cleanup before anything else.</p>

    <h2 id="settings">Your settings</h2>
    <p><em>Settings → Backup</em> exports your settings to a file, imports one, and keeps versioned backups on this PC (the newest 20). Importing or restoring always backs up your current settings first, so you can undo it.</p>
    <p>You can also sign in with Google to keep the same backups in a hidden folder of your own Google Drive. It is optional; see the <a href="/privacy">privacy policy</a>.</p>

    <h2 id="problems">When something goes wrong</h2>
    <ul>
      <li><strong>Windows says "protected your PC".</strong> The installer is not code-signed yet. Click <em>More info</em>, then <em>Run anyway</em>.</li>
      <li><strong>The Notifications page</strong> keeps the message of every failed install, in full.</li>
      <li><strong>A download stops moving.</strong> After a minute the launcher cancels it and takes you back to the link step so you can try again.</li>
      <li><strong>Mod folders out of place?</strong> Use the folder button next to a version to open it in the file manager.</li>
      <li><strong>Still stuck?</strong> <a href="{REPO}/issues" rel="noopener">Open an issue</a> and say which mod and version, and what the Notifications page says.</li>
    </ul>

    <h2 id="faq">Questions</h2>
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
