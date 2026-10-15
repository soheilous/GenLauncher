<script lang="ts">
  import legal from "$lib/generated/legal.json";
  import { getI18n } from "$lib/i18n";

  const { t, lang } = getI18n();
</script>

<svelte:head>
  <title>{t("Privacy policy · GenLauncher")}</title>
  <meta name="description" content={t("How GenLauncher handles data: no analytics, no server, and an optional Google Drive backup of your settings.")} />
</svelte:head>

<div class="wrap page">
  {#if lang !== "en"}
    <p class="english" role="note">{t("This document is available in English only, and the English version applies.")}</p>
  {/if}
  <div class="prose" lang="en" dir="ltr">{@html legal.privacy}</div>
</div>

<style>.page { padding-top: 3rem; } .prose :global(a) { overflow-wrap: anywhere; }  .english { padding: .8rem 1rem; border: 1px solid var(--stroke); border-radius: var(--r-md); background: var(--surface); color: var(--text-dim); margin-bottom: 1.5rem; }
</style>
