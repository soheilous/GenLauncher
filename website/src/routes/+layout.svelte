<script lang="ts">
  import "../app.css";
  import { page } from "$app/state";
  import { CATALOG, REPO } from "$lib/data";

  let { children } = $props();

  let theme = $state<"dark" | "light">("dark");
  $effect(() => { theme = (document.documentElement.dataset.theme as "dark" | "light") ?? "dark"; });
  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("theme", theme); } catch { /* private window */ }
  }

  const nav = [
    { href: "/#features", label: "Features" },
    { href: "/mods", label: "Mods" },
    { href: "/guide", label: "Guide" },
  ];
  const here = (href: string) => page.url.pathname === href.split("#")[0] && !href.includes("#");
</script>

<a class="skip" href="#main">Skip to the content</a>

<header class="bar">
  <div class="wrap row">
    <a class="brand" href="/" aria-label="GenLauncher, home"><img src="/logo.svg" alt="" width="34" height="34" /><span>GenLauncher</span></a>
    <nav aria-label="Main">
      {#each nav as n}<a href={n.href} aria-current={here(n.href) ? "page" : undefined}>{n.label}</a>{/each}
      <a href={REPO} rel="noopener">GitHub</a>
    </nav>
    <button class="icon" onclick={toggleTheme} aria-label="Switch to {theme === 'dark' ? 'light' : 'dark'} theme" title="Switch theme">
      {#if theme === "dark"}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM12 1v3M12 20v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M1 12h3M20 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/></svg>
      {:else}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z"/></svg>
      {/if}
    </button>
    <a class="btn primary get" href="/#get">Get it</a>
  </div>
</header>

<main id="main">{@render children()}</main>

<footer>
  <div class="wrap cols">
    <div class="about">
      <a class="brand" href="/"><img src="/logo.svg" alt="" width="30" height="30" /><span>GenLauncher</span></a>
      <p>A free, open-source mod launcher for Command &amp; Conquer: Generals and Zero Hour. A fan project, not affiliated with Electronic Arts, the authors of any mod, or Google. <em>Command &amp; Conquer</em> is a trademark of Electronic Arts Inc.</p>
    </div>
    <div>
      <h3>Explore</h3>
      <a href="/#features">Features</a><a href="/mods">Mods</a><a href="/guide">Guide &amp; FAQ</a><a href="/#get">Get GenLauncher</a>
    </div>
    <div>
      <h3>Project</h3>
      <a href={REPO} rel="noopener">Source on GitHub</a><a href="{REPO}/issues" rel="noopener">Report a problem</a><a href={CATALOG} rel="noopener">Improve the mod catalog</a>
    </div>
    <div>
      <h3>Legal</h3>
      <a href="/privacy">Privacy policy</a><a href="/terms">Terms of service</a>
    </div>
  </div>
</footer>

<style>
  .skip { position: absolute; left: -999px; top: .5rem; background: var(--surface-solid); padding: .5rem 1rem; border-radius: 8px; z-index: 100; }
  .skip:focus { left: .5rem; }
  .bar { position: sticky; top: 0; z-index: 50; backdrop-filter: blur(18px) saturate(150%); -webkit-backdrop-filter: blur(18px) saturate(150%);
    background: color-mix(in srgb, var(--bg) 72%, transparent); border-bottom: 1px solid var(--stroke); }
  .row { display: flex; align-items: center; gap: 1.2rem; height: 4rem; }
  .brand { display: inline-flex; align-items: center; gap: .65rem; color: var(--text); font-weight: 700; font-size: 1.1rem; text-decoration: none; }
  .brand img { border-radius: 9px; }
  nav { display: flex; gap: .3rem; margin-left: auto; }
  nav a { color: var(--text-dim); text-decoration: none; padding: .45rem .8rem; border-radius: 999px; font-weight: 500; }
  nav a:hover, nav a[aria-current="page"] { color: var(--text); background: var(--surface-hi); }
  .icon { display: grid; place-items: center; width: 2.4rem; height: 2.4rem; border-radius: 50%; border: 1px solid var(--stroke); background: var(--surface); color: var(--text); cursor: pointer; }
  .icon:hover { background: var(--surface-hi); }
  .get { padding: .5rem 1.1rem; }

  footer { margin-top: 6rem; border-top: 1px solid var(--stroke); background: var(--bg-2); padding: 3rem 0 2.5rem; }
  .cols { display: grid; gap: 2rem; grid-template-columns: 2fr 1fr 1fr 1fr; }
  .about p { color: var(--text-faint); font-size: .9rem; max-width: 26rem; margin: .9rem 0 0; }
  footer h3 { font-size: .8rem; text-transform: uppercase; letter-spacing: .09em; color: var(--text-faint); margin: .3rem 0 .8rem; }
  footer .cols > div:not(.about) a { display: block; color: var(--text-dim); text-decoration: none; padding: .22rem 0; }
  footer .cols > div:not(.about) a:hover { color: var(--accent); }

  @media (max-width: 820px) {
    .row { flex-wrap: wrap; height: auto; padding: .7rem 0; row-gap: .4rem; }
    nav { order: 5; width: 100%; margin: 0; overflow-x: auto; }
    .icon { margin-left: auto; }
    .cols { grid-template-columns: 1fr 1fr; }
    .about { grid-column: 1 / -1; }
  }
</style>
