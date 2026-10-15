<script lang="ts">
  import { faq } from "$lib/faq";
</script>

<div class="faq">
  {#each faq as item}
    <details>
      <summary>{item.q}</summary>
      <p>{item.a}</p>
    </details>
  {/each}
</div>

<style>
  .faq { display: grid; gap: .65rem; max-width: 50rem; }
  details { background: var(--surface); border: 1px solid var(--stroke); border-radius: var(--r-md); padding: 0 1.1rem; transition: border-color .2s; }
  details[open] { border-color: var(--stroke-hi); background: var(--surface-hi); }
  summary { cursor: pointer; font-weight: 600; padding: 1rem 0; list-style: none; display: flex; justify-content: space-between; gap: 1rem; align-items: center; }
  summary::-webkit-details-marker { display: none; }
  summary::after { content: "+"; font-size: 1.4rem; line-height: 1; color: var(--accent); transition: transform .2s var(--ease); }
  details[open] summary::after { transform: rotate(45deg); }
  p { margin: 0 0 1.1rem; color: var(--text-dim); }
</style>
