<script lang="ts">
  import legal from "$lib/generated/legal.json";
  import { getI18n } from "$lib/i18n";

  const { t, lang } = getI18n();
</script>

<svelte:head>
  <title>{t("Terms of service · GenLauncher")}</title>
  <meta name="description" content={t("The terms for using GenLauncher, a free fan-made launcher for Command & Conquer: Generals and Zero Hour.")} />
</svelte:head>

<div class="wrap page">
  {#if lang !== "en"}
    <p class="english" role="note">{t("This document is available in English only, and the English version applies.")}</p>
  {/if}
  <div class="prose" lang="en" dir="ltr">{@html legal.terms}</div>
</div>

<style>.page { padding-top: 3rem; } .prose :global(a) { overflow-wrap: anywhere; }  .english { padding: .8rem 1rem; border: 1px solid var(--stroke); border-radius: var(--r-md); background: var(--surface); color: var(--text-dim); margin-bottom: 1.5rem; }
</style>
