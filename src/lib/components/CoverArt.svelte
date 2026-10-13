<script lang="ts">
  import type { Snippet } from "svelte";
  import { art } from "$lib/art";
  import type { Item } from "$lib/types";

  // Repo covers are mostly logos with text, so they are shown whole (contain) over a blurred
  // copy of themselves instead of being cropped. Falls back to generated art if the image fails.
  let { item, pad = 0, children }: { item: Pick<Item, "cover" | "hue" | "name">; pad?: number; children?: Snippet } = $props();
  let failed = $state<string | null>(null);
  const src = $derived(item.cover && failed !== item.cover ? item.cover : null);
</script>

<div class="art" style:background={art(item.hue)}>
  {#if src}
    <div class="fill" style:background-image={`url("${src}")`}></div>
    <img {src} alt="" loading="lazy" style:padding-bottom="{pad}px" onerror={() => (failed = item.cover ?? null)} />
  {:else}
    <span class="initial" style:padding-bottom="{pad}px">{item.name[0]}</span>
  {/if}
  {@render children?.()}
</div>

<style>
  .art { position: absolute; inset: 0; overflow: hidden; display: grid; place-items: center; }
  .fill { position: absolute; inset: -20px; background-size: cover; background-position: center; filter: blur(22px) saturate(1.3) brightness(.8); }
  img { position: relative; width: 100%; height: 100%; object-fit: contain; padding: 10px; box-sizing: border-box; filter: drop-shadow(0 4px 14px rgb(0 0 0 / .45)); }
  .initial { font-size: 54px; font-weight: 800; color: rgb(255 255 255 / .85); text-shadow: 0 4px 20px rgb(0 0 0 / .35); }
</style>
