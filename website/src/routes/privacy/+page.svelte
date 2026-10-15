<script lang="ts">
  import legal from "$lib/generated/legal.json";
</script>

<svelte:head>
  <title>Privacy policy · GenLauncher</title>
  <meta name="description" content="How GenLauncher handles data: no analytics, no server, and an optional Google Drive backup of your settings." />
</svelte:head>

<div class="wrap page"><div class="prose">{@html legal.privacy}</div></div>

<style>.page { padding-top: 3rem; } .prose :global(a) { overflow-wrap: anywhere; }</style>
