<script lang="ts">
  import legal from "$lib/generated/legal.json";
</script>

<svelte:head>
  <title>Terms of service · GenLauncher</title>
  <meta name="description" content="The terms for using GenLauncher, a free fan-made launcher for Command & Conquer: Generals and Zero Hour." />
</svelte:head>

<div class="wrap page"><div class="prose">{@html legal.terms}</div></div>

<style>.page { padding-top: 3rem; } .prose :global(a) { overflow-wrap: anywhere; }</style>
