<script lang="ts">
  import ModCard from "$lib/ModCard.svelte";
  import Faq from "$lib/Faq.svelte";
  import Showcase from "$lib/Showcase.svelte";
  import Rich from "$lib/Rich.svelte";
  import { CATALOG, featured, release, stats, REPO } from "$lib/data";
  import { N, getI18n } from "$lib/i18n";

  const { t, href } = getI18n();

  // simple stroke icons (24x24 grid)
  const icons: Record<string, string> = {
    shield: "M12 3l8 3v6c0 4.5-3.2 8-8 9-4.8-1-8-4.5-8-9V6zM9 12l2 2 4-4",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    layers: "M12 3l9 5-9 5-9-5zM3 13l9 5 9-5M3 17.5l9 5 9-5",
    puzzle: "M10 4a2 2 0 1 1 4 0v2h4v4h-2a2 2 0 1 0 0 4h2v4h-4v-2a2 2 0 1 0-4 0v2H6v-4h2a2 2 0 1 0 0-4H6V6h4z",
    refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
    sliders: "M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M10 10v4M16 16v4",
    bell: "M6 16v-5a6 6 0 0 1 12 0v5l2 2H4zM10 21h4",
    cloud: "M7 18a4 4 0 0 1-.5-7.97A6 6 0 0 1 18 9a4.5 4.5 0 0 1 .5 9z",
    palette: "M12 3a9 9 0 1 0 0 18c1.5 0 2-1 1.5-2s0-2 1.5-2h2a4 4 0 0 0 4-4 9 9 0 0 0-9-10zM7.5 11h.01M10 7.5h.01M14 7.5h.01M16.5 11h.01",
  };
  const features = [
    { icon: "shield", title: N("Safe by design"), text: N("Every change to your game folder is written down first and undone when the game closes, even after a crash. Your original files are never edited.") },
    { icon: "link", title: N("Mods from their authors"), text: N("Nothing is mirrored. The launcher opens a mod's own page and takes the download you click, so files come from the people who made them.") },
    { icon: "layers", title: N("Every version, side by side"), text: N("Keep several versions of a mod and pick which one to play. Each remembers the page it came from.") },
    { icon: "puzzle", title: N("Patches, add-ons, executables"), text: N("Each mod has its own tabs. Switch on exactly the combination you want, one click each.") },
    { icon: "refresh", title: N("Check for newer versions"), text: N("One button reopens a version's page to fetch the next release. Same-name mods are never silently mixed up.") },
    { icon: "sliders", title: N("Game options built in"), text: N("Edit resolution, textures, particles and effects for the game without hunting for Options.ini.") },
    { icon: "bell", title: N("A notification page"), text: N("Install results, problems and project news are kept until you read them.") },
    { icon: "cloud", title: N("Backups that follow you"), text: N("Export your settings, keep versioned backups, or sync them to a hidden folder in your own Google Drive.") },
    { icon: "palette", title: N("Looks like it belongs"), text: N("Light and dark themes, glass panels, and an accent colour taken from each mod's art.") },
  ];
</script>

<svelte:head>
  <title>{t("GenLauncher: a mod launcher for Command & Conquer: Generals and Zero Hour")}</title>
  <meta name="description" content={t("Install, switch between and start Generals and Zero Hour mods, patches and add-ons. Your game folder is restored when the game closes. Free and open source.")} />
  <meta property="og:title" content="GenLauncher" />
  <meta property="og:description" content={t("A modern mod launcher for Command & Conquer: Generals and Zero Hour.")} />
  <meta property="og:image" content="/logo-512.png" />
</svelte:head>

<section class="hero">
  <div class="glow" aria-hidden="true"></div>
  <div class="wrap">
    <p class="badge"><span class="dot"></span> {t("Preview · Windows · Zero Hour & Generals")}</p>
    <h1>{t("Every Generals mod.")}<br /><span class="grad">{t("Your game left untouched.")}</span></h1>
    <p class="lead">{t("GenLauncher installs, updates and switches between mods, patches and add-ons, then starts the game with exactly the combination you chose. When the game closes, your folder is put back the way it was.")}</p>
    <div class="cta">
      <a class="btn primary" href="#get">{t("Get GenLauncher")}</a>
      <a class="btn" href="#try">{t("Try it live")}</a>
      <a class="btn" href={href("/mods")}>{t("Browse {n} mods", { n: stats.mods })}</a>
    </div>
    <ul class="chips" aria-label={t("Highlights")}>
      <li>{t("Free & open source")}</li><li>{t("No admin rights")}</li><li>{t("Dark & light theme")}</li><li>{t("Linux planned")}</li>
    </ul>
  </div>
</section>

<Showcase />

<section class="wrap metrics" aria-label={t("In numbers")}>
  <div><strong>{stats.mods}</strong><span>{t("mods in the catalog")}</span></div>
  <div><strong>{stats.patches + stats.addons}</strong><span>{t("patches & add-ons")}</span></div>
  <div><strong>2</strong><span>{t("games supported")}</span></div>
  <div><strong>0</strong><span>{t("files hosted by us")}</span></div>
</section>

<section class="wrap block" id="features" aria-labelledby="feat-h">
  <h2 id="feat-h">{t("Made for people who play with mods")}</h2>
  <p class="sub">{t("The old launcher, rebuilt from scratch: faster, safer and nicer to look at.")}</p>
  <div class="grid3">
    {#each features as f}
      <article class="feature">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={icons[f.icon]} /></svg>
        <h3>{t(f.title)}</h3>
        <p>{t(f.text)}</p>
      </article>
    {/each}
  </div>
</section>

<section class="wrap block" aria-labelledby="how-h">
  <h2 id="how-h">{t("Three steps to play")}</h2>
  <ol class="steps">
    <li><b>1</b><div><h3>{t("Point it at your game")}</h3><p>{t("GenLauncher finds your Steam or EA App install of Zero Hour or Generals. Already using the old launcher? It offers to move your mods over.")}</p></div></li>
    <li><b>2</b><div><h3>{t("Pick a mod")}</h3><p>{t("Press Install on a mod in Discover, choose the version on its own page and click download. Or add a zip, 7z, rar or folder you already have.")}</p></div></li>
    <li><b>3</b><div><h3>{t("Press Play")}</h3><p>{t("Choose the patches and add-ons you want and play. When you quit, the game folder goes back to normal.")}</p></div></li>
  </ol>
</section>

<section class="wrap block" aria-labelledby="mods-h">
  <div class="head"><h2 id="mods-h">{t("A growing mod catalog")}</h2><a href={href("/mods")}>{t("See all {n} mods →", { n: stats.mods })}</a></div>
  <div class="grid4">
    {#each featured as m (m.name)}<ModCard mod={m} />{/each}
  </div>
  <p class="note"><Rich text={t("The catalog only points to each mod's page; every file comes from its author. Missing a mod or found a dead link? [Improve the catalog on GitHub.]({url})", { url: CATALOG })} /></p>
</section>

<section class="wrap block get" id="get" aria-labelledby="get-h">
  <div class="card">
    <h2 id="get-h">{t("Get GenLauncher")}</h2>
    {#if release}
      <p class="status"><span class="dot"></span> {release.prerelease
        ? t("Version {version} (preview) · Windows 10 and 11 · {size} MB", { version: release.version, size: (release.size / 1048576).toFixed(1) })
        : t("Version {version} · Windows 10 and 11 · {size} MB", { version: release.version, size: (release.size / 1048576).toFixed(1) })}</p>
      <p>{t("A normal setup program: it installs for your user account and needs no administrator rights.")}</p>
      <div class="cta">
        <a class="btn primary" href={release.download} rel="noopener">{t("Download for Windows")}</a>
        <a class="btn" href={release.page} rel="noopener">{t("Release notes")}</a>
        <a class="btn" href="{REPO}#development">{t("Build it yourself")}</a>
      </div>
      <p class="warn-note"><Rich text={t("**Windows may say \"Windows protected your PC\".** The installer is not code-signed yet, which is what triggers it: click *More info*, then *Run anyway*. See the [guide](/guide#problems).")} /></p>
    {:else}
      <p class="status"><span class="dot"></span> {t("Preview for Windows")}</p>
      <p>{t("GenLauncher is under active development. Get the installer from the releases page, or build it yourself; the README explains how.")}</p>
      <div class="cta">
        <a class="btn primary" href="{REPO}/releases" rel="noopener">{t("Releases on GitHub")}</a>
        <a class="btn" href="{REPO}#development">{t("Build it yourself")}</a>
      </div>
    {/if}
    <p class="req"><Rich text={t("**Needs:** Windows 10 or 11 and a copy of Generals or Zero Hour (Steam, EA App or disc). Linux is planned.")} /></p>
  </div>
</section>

<section class="wrap block" aria-labelledby="faq-h">
  <h2 id="faq-h">{t("Questions")}</h2>
  <Faq />
  <p class="note"><Rich text={t("More in the [guide](/guide).")} /></p>
</section>

<section class="wrap block thanks" aria-labelledby="thanks-h">
  <h2 id="thanks-h">{t("Built on other people's work")}</h2>
  <p><Rich text={t("GenLauncher continues the original [GenLauncher](https://github.com/p0ls3r/GenLauncher) by **p0ls3r** and its contributors, whose mod list at [GenLauncherModsData](https://github.com/p0ls3r/GenLauncherModsData) was the starting point of our index. Every mod listed belongs to its authors. Thank you for keeping Generals alive.")} /></p>
</section>

<style>
  .hero { position: relative; padding: 5rem 0 3rem; overflow: hidden; text-align: center; }
  .glow { position: absolute; inset: -10% -10% auto; height: 34rem; z-index: -1;
    background: radial-gradient(40rem 20rem at 30% 20%, var(--glow-1), transparent 70%), radial-gradient(34rem 18rem at 75% 10%, var(--glow-2), transparent 70%); }
  .badge { display: inline-flex; align-items: center; gap: .55rem; margin: 0 0 1.4rem; padding: .4rem .95rem; border-radius: 999px; border: 1px solid var(--stroke-hi); background: var(--surface); color: var(--text-dim); font-size: .9rem; }
  .dot { width: .55rem; height: .55rem; border-radius: 50%; background: var(--accent-2); box-shadow: 0 0 10px var(--accent-2); }
  h1 { font-size: clamp(2.3rem, 6vw, 4.2rem); line-height: 1.05; letter-spacing: -.035em; margin: 0 0 1.2rem; }
  .grad { background: linear-gradient(135deg, var(--accent), var(--accent-2)); -webkit-background-clip: text; background-clip: text; color: transparent; }
  .lead { max-width: 40rem; margin: 0 auto 1.8rem; font-size: 1.2rem; color: var(--text-dim); }
  .cta { display: flex; flex-wrap: wrap; gap: .8rem; justify-content: center; }
  .chips { list-style: none; display: flex; flex-wrap: wrap; gap: .5rem 1.4rem; justify-content: center; padding: 0; margin: 1.6rem 0 0; color: var(--text-faint); font-size: .92rem; }
  .chips li::before { content: "✓ "; color: var(--accent); }

  .metrics { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin-top: 2.5rem; }
  .metrics div { background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); padding: 1.1rem 1rem; text-align: center; }
  .metrics strong { display: block; font-size: 2rem; line-height: 1.1; background: linear-gradient(135deg, var(--accent), var(--accent-2)); -webkit-background-clip: text; background-clip: text; color: transparent; }
  .metrics span { color: var(--text-dim); font-size: .92rem; }

  .block { margin-top: 6rem; }
  .block h2 { font-size: clamp(1.7rem, 3.4vw, 2.3rem); letter-spacing: -.02em; line-height: 1.15; margin: 0 0 .6rem; }
  .sub { color: var(--text-dim); margin: 0 0 2rem; font-size: 1.1rem; }
  .grid3 { display: grid; gap: 1rem; grid-template-columns: repeat(3, 1fr); }
  .feature { background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); padding: 1.3rem 1.3rem 1.4rem; transition: border-color .25s, transform .25s var(--ease); }
  .feature:hover { border-color: var(--stroke-hi); transform: translateY(-2px); }
  .feature svg { color: var(--accent); }
  .feature h3 { margin: .8rem 0 .35rem; font-size: 1.1rem; }
  .feature p { margin: 0; color: var(--text-dim); font-size: .97rem; }

  .steps { list-style: none; padding: 0; margin: 1.6rem 0 0; display: grid; gap: 1rem; grid-template-columns: repeat(3, 1fr); }
  .steps li { display: flex; gap: 1rem; background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); padding: 1.3rem; }
  .steps b { flex: none; width: 2.2rem; height: 2.2rem; border-radius: 50%; display: grid; place-items: center; color: var(--accent-ink); background: linear-gradient(135deg, var(--accent), var(--accent-2)); }
  .steps h3 { margin: .1rem 0 .3rem; font-size: 1.08rem; }
  .steps p { margin: 0; color: var(--text-dim); font-size: .96rem; }

  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 1rem; flex-wrap: wrap; margin-bottom: 1.4rem; }
  .head h2 { margin: 0; }
  .grid4 { display: grid; gap: 1rem; grid-template-columns: repeat(4, 1fr); }
  .note { color: var(--text-faint); font-size: .93rem; margin-top: 1.2rem; }

  .get .card { background: linear-gradient(135deg, var(--surface-hi), var(--surface)); border: 1px solid var(--stroke-hi); border-radius: var(--r-lg); padding: 2.2rem; text-align: center; box-shadow: 0 0 80px var(--glow-2); }
  .get .card p { color: var(--text-dim); max-width: 38rem; margin-inline: auto; }
  .get .status { display: inline-flex; align-items: center; gap: .6rem; color: var(--text); font-weight: 600; }
  .get .req { margin-top: 1.4rem; font-size: .95rem; }
  .get .warn-note { margin-top: 1.1rem; font-size: .92rem; background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); padding: .8rem 1rem; text-align: start; }

  .thanks p { color: var(--text-dim); max-width: 46rem; }

  @media (max-width: 980px) { .grid4 { grid-template-columns: repeat(2, 1fr); } .grid3 { grid-template-columns: repeat(2, 1fr); } .steps { grid-template-columns: 1fr; } }
  @media (max-width: 620px) {
    .hero { padding-top: 3rem; } .grid4, .grid3 { grid-template-columns: 1fr; } .metrics { grid-template-columns: repeat(2, 1fr); }
    .get .card { padding: 1.5rem 1.1rem; }
  }
</style>
