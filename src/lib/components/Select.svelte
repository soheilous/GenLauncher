<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Option { value: string; label: string; hint?: string; muted?: boolean }
  let { options, value, onchange, label }: {
    options: Option[]; value: string; onchange: (v: string) => void; label: string;
  } = $props();

  let open = $state(false);
  let active = $state(0);
  let root: HTMLDivElement | undefined = $state();
  const current = $derived(options.find((o) => o.value === value) ?? options[0]);

  function toggle() {
    open = !open;
    if (open) active = Math.max(0, options.findIndex((o) => o.value === value));
  }
  function pick(o: Option) { onchange(o.value); open = false; }

  function onkey(e: KeyboardEvent) {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) { e.preventDefault(); toggle(); }
      return;
    }
    if (e.key === "Escape") { e.preventDefault(); open = false; }
    else if (e.key === "ArrowDown") { e.preventDefault(); active = (active + 1) % options.length; }
    else if (e.key === "ArrowUp") { e.preventDefault(); active = (active - 1 + options.length) % options.length; }
    else if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(options[active]); }
    else if (e.key === "Tab") open = false;
  }

  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => { if (root && !root.contains(e.target as Node)) open = false; };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  });
</script>

<div class="sel" bind:this={root}>
  <button class="trigger" class:open aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={toggle} onkeydown={onkey}>
    <span class="cur">{current?.label}</span>
    {#if current?.hint}<span class="hint" class:warn={current.muted}>{current.hint}</span>{/if}
    <span class="chev" class:flip={open}><Icon name="chevron" size={14} /></span>
  </button>

  {#if open}
    <ul class="menu" role="listbox" aria-label={label}>
      {#each options as o, i (o.value)}
        <li role="option" aria-selected={o.value === value} class:active={i === active} class:sel={o.value === value}
          onpointerenter={() => (active = i)} onclick={() => pick(o)} onkeydown={onkey} tabindex="-1">
          <span class="name" class:muted={o.muted}>{o.label}</span>
          {#if o.hint}<span class="hint" class:warn={o.muted}>{o.hint}</span>{/if}
          {#if o.value === value}<Icon name="check" size={14} />{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sel { position: relative; flex: 1; min-width: 0; }
  .trigger { width: 100%; display: flex; align-items: center; gap: 8px; padding: 8px 12px 8px 14px; border-radius: 999px; text-align: left;
    background: var(--surface-hi); border: 1px solid var(--stroke); transition: border-color .18s, background .18s; }
  .trigger:hover, .trigger.open { border-color: var(--stroke-hi); }
  .trigger.open { border-color: var(--accent); }
  .trigger:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cur { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { font-size: 11px; font-weight: 600; color: var(--ok); }
  .hint.warn { color: var(--warn); }
  .chev { display: grid; color: var(--text-faint); transition: transform .2s var(--ease); }
  .chev.flip { transform: rotate(180deg); }

  .menu { position: absolute; z-index: 15; left: 0; right: 0; top: calc(100% + 6px); margin: 0; padding: 6px; list-style: none;
    max-height: 240px; overflow-y: auto; border-radius: var(--r-md);
    background: var(--surface-solid); border: 1px solid var(--stroke-hi); box-shadow: var(--shadow);
    backdrop-filter: var(--blur); animation: drop .16s var(--ease); transform-origin: top; }
  li { display: flex; align-items: center; gap: 8px; padding: 9px 10px; border-radius: 10px; cursor: pointer; color: var(--text-dim); transition: background .12s; }
  li.active { background: var(--surface-hi); color: var(--text); }
  li.sel { color: var(--accent); font-weight: 600; }
  li .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  li .name.muted { opacity: .75; }
  @keyframes drop { from { opacity: 0; transform: translateY(-6px) scaleY(.96); } }
</style>
