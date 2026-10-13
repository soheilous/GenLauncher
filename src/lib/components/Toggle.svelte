<script lang="ts">
  let { checked, onchange, label }: { checked: boolean; onchange: (v: boolean) => void; label: string } = $props();
</script>

<button class="toggle" role="switch" aria-checked={checked} aria-label={label} class:on={checked} onclick={() => onchange(!checked)}>
  <span></span>
</button>

<style>
  .toggle { width: 42px; height: 24px; border-radius: 999px; background: var(--surface-hi); border: 1px solid var(--stroke); padding: 2px; transition: background .2s; flex: none; }
  .toggle span { display: block; width: 18px; height: 18px; border-radius: 50%; background: var(--text-dim); transition: transform .22s var(--ease), background .2s; }
  .toggle.on { background: var(--accent); border-color: transparent; }
  .toggle.on span { transform: translateX(18px); background: var(--accent-ink); }
  .toggle:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
