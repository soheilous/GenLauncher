<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { CATALOG, REPO } from "$lib/data";
  import { LANGUAGES, N, OTHER_LANGUAGES, localize, provideI18n, switchLanguage, type Messages } from "$lib/i18n";
  import Rich from "$lib/Rich.svelte";

  let { data, children } = $props();
  const i18n = provideI18n(() => ({ lang: data.lang, messages: data.messages as Messages }));
  const t = (text: string, vars?: Record<string, string | number>) => i18n.t(text, vars);

  let theme = $state<"dark" | "light">("dark");
  $effect(() => { theme = (document.documentElement.dataset.theme as "dark" | "light") ?? "dark"; });
  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("theme", theme); } catch { /* private window */ }
  }

  const nav = [
    { href: "/#features", label: N("Features") },
    { href: "/#try", label: N("Try it") },
    { href: "/mods", label: N("Mods") },
    { href: "/guide", label: N("Guide") },
  ];
  const here = (href: string) => !href.includes("#") && page.url.pathname.replace(/\/$/, "") === i18n.href(href).replace(/\/$/, "");

  const current = $derived(LANGUAGES.find((l) => l.code === data.lang) ?? LANGUAGES[0]);
  let menu = $state<HTMLDetailsElement>();

  const SUGGEST = N("This page is also available in {language}.");
  const DISMISS = N("Dismiss");
  // an English page offers the visitor's own language once, in that language
  let suggestion = $state<{ code: string; dir: string; text: string; dismiss: string } | null>(null);
  onMount(async () => {
    if (data.lang !== "en") return;
    try { if (localStorage.getItem("lang-suggestion") === "no") return; } catch { /* private window */ }
    const asked = (navigator.languages?.length ? navigator.languages : [navigator.language]).map((l) => (l ?? "").toLowerCase().split("-")[0]);
    const code = asked.find((l) => OTHER_LANGUAGES.includes(l));
    if (!code) return;
    const m: Messages = (await import(`../lib/locales/${code}.ts`)).default;
    const text = m[SUGGEST];
    // no translation yet: do not show English in a foreign-language bar
    if (typeof text !== "string") return;
    const lang = LANGUAGES.find((l) => l.code === code)!;
    const dismiss = m[DISMISS];
    suggestion = { code, dir: lang.dir, text: text.replace("{language}", lang.name), dismiss: typeof dismiss === "string" ? dismiss : "×" };
  });
  function dismissSuggestion() {
    suggestion = null;
    try { localStorage.setItem("lang-suggestion", "no"); } catch { /* private window */ }
  }
</script>

<svelte:window
  onclick={(e) => { if (menu?.open && !menu.contains(e.target as Node)) menu.open = false; }}
  onkeydown={(e) => { if (e.key === "Escape" && menu) menu.open = false; }}
/>

<svelte:head>
  <meta property="og:locale" content={data.lang} />
  {#if data.origin}
    <link rel="canonical" href={data.origin + page.url.pathname} />
    {#each LANGUAGES as l}<link rel="alternate" hreflang={l.code} href={data.origin + switchLanguage(page.url.pathname, l.code)} />{/each}
    <link rel="alternate" hreflang="x-default" href={data.origin + switchLanguage(page.url.pathname, "en")} />
  {/if}
</svelte:head>

{#if suggestion}
  <div class="suggest" role="note" lang={suggestion.code} dir={suggestion.dir as "ltr" | "rtl"}>
    <a href={localize(suggestion.code, "/")} hreflang={suggestion.code}>{suggestion.text}</a>
    <button onclick={dismissSuggestion} aria-label={suggestion.dismiss} title={suggestion.dismiss}>×</button>
  </div>
{/if}

<a class="skip" href="#main">{t("Skip to the content")}</a>

<header class="bar">
  <div class="wrap row">
    <a class="brand" href={i18n.href("/")} aria-label={t("GenLauncher, home")}><img src="/logo.svg" alt="" width="34" height="34" /><span>GenLauncher</span></a>
    <nav aria-label={t("Main")}>
      {#each nav as n}<a href={i18n.href(n.href)} aria-current={here(n.href) ? "page" : undefined}>{t(n.label)}</a>{/each}
      <a href={REPO} rel="noopener">GitHub</a>
    </nav>
    <details class="lang" bind:this={menu}>
      <summary aria-label={t("Language")}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.5 3.5 5.5 3.5 9s-1 6.5-3.5 9c-2.5-2.5-3.5-5.5-3.5-9S9.5 5.5 12 3z"/></svg>
        <span>{current.name}</span>
      </summary>
      <ul>
        {#each LANGUAGES as l}
          <li><a href={switchLanguage(page.url.pathname, l.code)} lang={l.code} hreflang={l.code} aria-current={l.code === data.lang ? "true" : undefined} data-sveltekit-reload>{l.name}</a></li>
        {/each}
      </ul>
    </details>
    <button class="icon" onclick={toggleTheme} aria-label={theme === "dark" ? t("Switch to the light theme") : t("Switch to the dark theme")} title={t("Switch theme")}>
      {#if theme === "dark"}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10zM12 1v3M12 20v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M1 12h3M20 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/></svg>
      {:else}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z"/></svg>
      {/if}
    </button>
    <a class="btn primary get" href={i18n.href("/#get")}>{t("Get it")}</a>
  </div>
</header>

<main id="main">{@render children()}</main>

<footer>
  <div class="wrap cols">
    <div class="about">
      <a class="brand" href={i18n.href("/")}><img src="/logo.svg" alt="" width="30" height="30" /><span>GenLauncher</span></a>
      <p><Rich text={t("A free, open-source mod launcher for Command & Conquer: Generals and Zero Hour. A fan project, not affiliated with Electronic Arts, the authors of any mod, or Google. *Command & Conquer* is a trademark of Electronic Arts Inc.")} /></p>
      {#if data.lang !== "en"}
        <p><Rich text={t("This translation was made with care and may have mistakes. [Help improve it.]({url})", { url: `${REPO}/edit/main/website/src/lib/locales/${data.lang}.ts` })} /></p>
      {/if}
    </div>
    <div>
      <h3>{t("Explore")}</h3>
      <a href={i18n.href("/#features")}>{t("Features")}</a><a href={i18n.href("/mods")}>{t("Mods")}</a><a href={i18n.href("/guide")}>{t("Guide & FAQ")}</a><a href={i18n.href("/#get")}>{t("Get GenLauncher")}</a>
    </div>
    <div>
      <h3>{t("Project")}</h3>
      <a href={REPO} rel="noopener">{t("Source on GitHub")}</a><a href="{REPO}/issues" rel="noopener">{t("Report a problem")}</a><a href={CATALOG} rel="noopener">{t("Improve the mod catalog")}</a>
    </div>
    <div>
      <h3>{t("Legal")}</h3>
      <a href={i18n.href("/privacy")}>{t("Privacy policy")}</a><a href={i18n.href("/terms")}>{t("Terms of service")}</a>
    </div>
  </div>
</footer>

<style>
  .skip { position: absolute; inset-inline-start: -999px; top: .5rem; background: var(--surface-solid); padding: .5rem 1rem; border-radius: 8px; z-index: 100; }
  .skip:focus { inset-inline-start: .5rem; }
  .suggest { display: flex; align-items: center; justify-content: center; gap: .8rem; padding: .45rem 1rem; background: linear-gradient(135deg, var(--accent), var(--accent-2)); color: var(--accent-ink); font-size: .92rem; font-weight: 600; }
  .suggest a { color: inherit; text-decoration: underline; text-underline-offset: 3px; }
  .suggest button { background: none; border: 0; color: inherit; font-size: 1.2rem; line-height: 1; cursor: pointer; padding: 0 .3rem; }
  .bar { position: sticky; top: 0; z-index: 50; backdrop-filter: blur(18px) saturate(150%); -webkit-backdrop-filter: blur(18px) saturate(150%);
    background: color-mix(in srgb, var(--bg) 72%, transparent); border-bottom: 1px solid var(--stroke); }
  .row { display: flex; align-items: center; gap: 1rem; height: 4rem; }
  .brand { display: inline-flex; align-items: center; gap: .65rem; color: var(--text); font-weight: 700; font-size: 1.1rem; text-decoration: none; }
  .brand img { border-radius: 9px; }
  nav { display: flex; gap: .3rem; margin-inline-start: auto; }
  nav a { color: var(--text-dim); text-decoration: none; padding: .45rem .8rem; border-radius: 999px; font-weight: 500; white-space: nowrap; }
  nav a:hover, nav a[aria-current="page"] { color: var(--text); background: var(--surface-hi); }
  .icon { display: grid; place-items: center; width: 2.4rem; height: 2.4rem; border-radius: 50%; border: 1px solid var(--stroke); background: var(--surface); color: var(--text); cursor: pointer; }
  .icon:hover { background: var(--surface-hi); }
  .get { padding: .5rem 1.1rem; white-space: nowrap; }

  .lang { position: relative; }
  .lang summary { list-style: none; display: inline-flex; align-items: center; gap: .45rem; height: 2.4rem; padding: 0 .9rem; border-radius: 999px; border: 1px solid var(--stroke); background: var(--surface); color: var(--text); cursor: pointer; font-size: .92rem; }
  .lang summary::-webkit-details-marker { display: none; }
  .lang summary:hover { background: var(--surface-hi); }
  .lang ul { position: absolute; inset-inline-end: 0; top: calc(100% + .4rem); z-index: 60; list-style: none; margin: 0; padding: .4rem; min-width: 11rem; max-height: 70vh; overflow-y: auto;
    background: var(--surface-solid); border: 1px solid var(--stroke-hi, var(--stroke)); border-radius: 14px; box-shadow: 0 12px 40px rgb(0 0 0 / .35); }
  .lang li a { display: block; padding: .5rem .8rem; border-radius: 10px; color: var(--text-dim); text-decoration: none; }
  .lang li a:hover { background: var(--surface-hi); color: var(--text); }
  .lang li a[aria-current="true"] { color: var(--accent); font-weight: 700; }

  footer { margin-top: 6rem; border-top: 1px solid var(--stroke); background: var(--bg-2); padding: 3rem 0 2.5rem; }
  .cols { display: grid; gap: 2rem; grid-template-columns: 2fr 1fr 1fr 1fr; }
  .about p { color: var(--text-faint); font-size: .9rem; max-width: 26rem; margin: .9rem 0 0; }
  footer h3 { font-size: .8rem; text-transform: uppercase; letter-spacing: .09em; color: var(--text-faint); margin: .3rem 0 .8rem; }
  footer .cols > div:not(.about) a { display: block; color: var(--text-dim); text-decoration: none; padding: .22rem 0; }
  footer .cols > div:not(.about) a:hover { color: var(--accent); }

  @media (max-width: 820px) {
    .row { flex-wrap: wrap; height: auto; padding: .7rem 0; row-gap: .4rem; }
    nav { order: 5; width: 100%; margin: 0; overflow-x: auto; }
    .lang { margin-inline-start: auto; }
    .cols { grid-template-columns: 1fr 1fr; }
    .about { grid-column: 1 / -1; }
  }
</style>
