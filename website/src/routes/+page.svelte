<script lang="ts">
  import ModCard from "$lib/ModCard.svelte";
  import Faq from "$lib/Faq.svelte";
  import { featured, stats, REPO } from "$lib/data";

  const shots = [
    { id: "library", label: "Library", text: "Your mods, each with its own art and accent colour. Pick a version, switch patches and add-ons on or off, and press Play." },
    { id: "discover", label: "Discover", text: "Browse the mod index. Install opens the mod's own page in a built-in browser: you choose the version and click download, the launcher does the rest." },
    { id: "notifications", label: "Notifications", text: "Nothing gets lost: finished installs, problems and messages from the project stay in one place until you dismiss them." },
    { id: "options", label: "Game options", text: "Resolution, texture quality, particle limit and effects, written straight into the game's own settings file, with a backup of the original." },
  ];
  let tab = $state(0);

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
    { icon: "shield", title: "Safe by design", text: "Every change to your game folder is written down first and undone when the game closes, even after a crash. Your original files are never edited." },
    { icon: "link", title: "Mods from their authors", text: "Nothing is mirrored. The launcher opens a mod's own page and takes the download you click, so files come from the people who made them." },
    { icon: "layers", title: "Every version, side by side", text: "Keep several versions of a mod and pick which one to play. Each remembers the page it came from." },
    { icon: "puzzle", title: "Patches, add-ons, executables", text: "Each mod has its own tabs. Switch on exactly the combination you want, one click each." },
    { icon: "refresh", title: "Check for newer versions", text: "One button reopens a version's page to fetch the next release. Same-name mods are never silently mixed up." },
    { icon: "sliders", title: "Game options built in", text: "Edit resolution, textures, particles and effects for the game without hunting for Options.ini." },
    { icon: "bell", title: "A notification page", text: "Install results, problems and project news are kept until you read them." },
    { icon: "cloud", title: "Backups that follow you", text: "Export your settings, keep versioned backups, or sync them to a hidden folder in your own Google Drive." },
    { icon: "palette", title: "Looks like it belongs", text: "Light and dark themes, glass panels, and an accent colour taken from each mod's art." },
  ];
</script>

<svelte:head>
  <title>GenLauncher: a mod launcher for Command &amp; Conquer: Generals and Zero Hour</title>
  <meta name="description" content="Install, switch between and start Generals and Zero Hour mods, patches and add-ons. Your game folder is restored when the game closes. Free and open source." />
  <meta property="og:title" content="GenLauncher" />
  <meta property="og:description" content="A modern mod launcher for Command & Conquer: Generals and Zero Hour." />
  <meta property="og:image" content="/logo-512.png" />
</svelte:head>

<section class="hero">
  <div class="glow" aria-hidden="true"></div>
  <div class="wrap">
    <p class="badge"><span class="dot"></span> Preview · Windows · Zero Hour &amp; Generals</p>
    <h1>Every Generals mod.<br /><span class="grad">Your game left untouched.</span></h1>
    <p class="lead">GenLauncher installs, updates and switches between mods, patches and add-ons, then starts the game with exactly the combination you chose. When the game closes, your folder is put back the way it was.</p>
    <div class="cta">
      <a class="btn primary" href="#get">Get GenLauncher</a>
      <a class="btn" href="/mods">Browse {stats.mods} mods</a>
    </div>
    <ul class="chips" aria-label="Highlights">
      <li>Free &amp; open source</li><li>No admin rights</li><li>Dark &amp; light theme</li><li>Linux planned</li>
    </ul>
  </div>
</section>

<section class="wrap showcase" aria-labelledby="show-h">
  <h2 id="show-h" class="sr-only">The launcher</h2>
  <div class="tabs" role="tablist" aria-label="Screens">
    {#each shots as s, i}
      <button role="tab" id="tab-{s.id}" aria-selected={tab === i} aria-controls="panel" class:on={tab === i} onclick={() => (tab = i)}>{s.label}</button>
    {/each}
  </div>
  <div class="window" role="tabpanel" id="panel" aria-labelledby="tab-{shots[tab].id}">
    <div class="chrome" aria-hidden="true"><i></i><i></i><i></i><span>GenLauncher</span></div>
    {#each shots as s, i}
      {#if i === tab}
        <img class="shot dark" src="/screens/{s.id}-dark.jpg" alt="The {s.label} screen of GenLauncher" width="1360" height="850" />
        <img class="shot light" src="/screens/{s.id}-light.jpg" alt="The {s.label} screen of GenLauncher" width="1360" height="850" />
      {/if}
    {/each}
  </div>
  <p class="caption">{shots[tab].text}</p>
</section>

<section class="wrap metrics" aria-label="In numbers">
  <div><strong>{stats.mods}</strong><span>mods in the index</span></div>
  <div><strong>{stats.patches + stats.addons}</strong><span>patches &amp; add-ons</span></div>
  <div><strong>2</strong><span>games supported</span></div>
  <div><strong>0</strong><span>files hosted by us</span></div>
</section>

<section class="wrap block" id="features" aria-labelledby="feat-h">
  <h2 id="feat-h">Made for people who play with mods</h2>
  <p class="sub">The old launcher, rebuilt from scratch: faster, safer and nicer to look at.</p>
  <div class="grid3">
    {#each features as f}
      <article class="feature">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={icons[f.icon]} /></svg>
        <h3>{f.title}</h3>
        <p>{f.text}</p>
      </article>
    {/each}
  </div>
</section>

<section class="wrap block" aria-labelledby="how-h">
  <h2 id="how-h">Three steps to play</h2>
  <ol class="steps">
    <li><b>1</b><div><h3>Point it at your game</h3><p>GenLauncher finds your Steam or EA App install of Zero Hour or Generals. Already using the old launcher? It offers to move your mods over.</p></div></li>
    <li><b>2</b><div><h3>Pick a mod</h3><p>Press Install on a mod in Discover, choose the version on its own page and click download. Or add a zip, 7z, rar or folder you already have.</p></div></li>
    <li><b>3</b><div><h3>Press Play</h3><p>Choose the patches and add-ons you want and play. When you quit, the game folder goes back to normal.</p></div></li>
  </ol>
</section>

<section class="wrap block" aria-labelledby="mods-h">
  <div class="head"><h2 id="mods-h">A growing mod index</h2><a href="/mods">See all {stats.mods} mods →</a></div>
  <div class="grid4">
    {#each featured as m (m.name)}<ModCard mod={m} />{/each}
  </div>
  <p class="note">The index only points to each mod's page; every file comes from its author. Missing a mod or found a dead link? <a href="{REPO}/tree/main/index" rel="noopener">Improve the index on GitHub.</a></p>
</section>

<section class="wrap block get" id="get" aria-labelledby="get-h">
  <div class="card">
    <h2 id="get-h">Get GenLauncher</h2>
    <p class="status"><span class="dot"></span> Preview: the first installer is not published yet.</p>
    <p>GenLauncher is under active development. Until the first release is out you can follow the project on GitHub or build it yourself; the README explains how.</p>
    <div class="cta">
      <a class="btn primary" href={REPO} rel="noopener">View on GitHub</a>
      <a class="btn" href="{REPO}#development">Build it yourself</a>
    </div>
    <p class="req"><strong>Needs:</strong> Windows 10 or 11 and a copy of Generals or Zero Hour (Steam, EA App or disc). Linux is planned.</p>
  </div>
</section>

<section class="wrap block" aria-labelledby="faq-h">
  <h2 id="faq-h">Questions</h2>
  <Faq />
  <p class="note">More in the <a href="/guide">guide</a>.</p>
</section>

<section class="wrap block thanks" aria-labelledby="thanks-h">
  <h2 id="thanks-h">Built on other people's work</h2>
  <p>GenLauncher continues the original <a href="https://github.com/p0ls3r/GenLauncher" rel="noopener">GenLauncher</a> by <strong>p0ls3r</strong> and its contributors, whose mod list at <a href="https://github.com/p0ls3r/GenLauncherModsData" rel="noopener">GenLauncherModsData</a> was the starting point of our index. Every mod listed belongs to its authors. Thank you for keeping Generals alive.</p>
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

  .showcase { text-align: center; }
  .tabs { display: inline-flex; flex-wrap: wrap; justify-content: center; gap: .3rem; padding: .3rem; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); margin-bottom: 1.3rem; }
  .tabs button { font: inherit; font-weight: 600; font-size: .95rem; color: var(--text-dim); background: none; border: 0; padding: .5rem 1.1rem; border-radius: 999px; cursor: pointer; transition: background .2s, color .2s; }
  .tabs button:hover { color: var(--text); }
  .tabs button.on { background: var(--accent); color: var(--accent-ink); }
  .window { border-radius: 18px; border: 1px solid var(--stroke-hi); background: var(--surface-solid); box-shadow: var(--shadow), 0 0 90px var(--glow-1); overflow: hidden; text-align: left; }
  .chrome { display: flex; align-items: center; gap: .45rem; padding: .6rem .9rem; border-bottom: 1px solid var(--stroke); background: var(--bg-2); }
  .chrome i { width: .7rem; height: .7rem; border-radius: 50%; background: var(--stroke-hi); }
  .chrome span { margin: 0 auto; transform: translateX(-1rem); font-size: .8rem; color: var(--text-faint); }
  .shot { width: 100%; height: auto; }
  :global(:root[data-theme="light"]) .shot.dark, :global(:root:not([data-theme="light"])) .shot.light { display: none; }
  .caption { margin: 1.1rem auto 0; max-width: 40rem; color: var(--text-dim); min-height: 3.4rem; }

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

  .thanks p { color: var(--text-dim); max-width: 46rem; }

  @media (max-width: 980px) { .grid4 { grid-template-columns: repeat(2, 1fr); } .grid3 { grid-template-columns: repeat(2, 1fr); } .steps { grid-template-columns: 1fr; } }
  @media (max-width: 620px) {
    .hero { padding-top: 3rem; } .grid4, .grid3 { grid-template-columns: 1fr; } .metrics { grid-template-columns: repeat(2, 1fr); }
    .get .card { padding: 1.5rem 1.1rem; }
  }
</style>
