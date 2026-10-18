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

  let trigger: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLUListElement | undefined = $state();
  /** The menu lives directly in <body>: inside a dialog that is centred with a transform, "fixed" would be relative to the dialog, not the screen. */
  function toBody(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
  const inside = (t: EventTarget | null) => t instanceof Node && (!!root?.contains(t) || !!menuEl?.contains(t));
  /** Where the menu goes. It is placed on the screen (fixed) from the button's position, so it never makes a page or a panel scroll. */
  let place = $state("");
  function position() {
    if (!trigger) return;
    const r = trigger.getBoundingClientRect();
    const below = innerHeight - r.bottom - 12;
    const above = r.top - 12;
    const down = below >= Math.min(240, options.length * 42 + 16) || below >= above;
    const rtl = getComputedStyle(trigger).direction === "rtl";
    place = [
      "position: fixed",
      down ? `top: ${r.bottom + 6}px` : `bottom: ${innerHeight - r.top + 6}px`,
      rtl ? `right: ${Math.max(8, innerWidth - r.right)}px` : `left: ${Math.max(8, r.left)}px`,
      `min-width: ${r.width}px`,
      `max-height: ${Math.max(120, Math.min(240, down ? below : above))}px`,
    ].join("; ");
  }
  function toggle() {
    open = !open;
    if (open) {
      active = Math.max(0, options.findIndex((o) => o.value === value));
      // a dialog may still be sliding into place when its select is opened: follow the button for a moment
      const start = performance.now();
      const follow = () => { position(); if (open && performance.now() - start < 400) requestAnimationFrame(follow); };
      follow();
    }
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
    const close = (e: PointerEvent) => { if (!inside(e.target)) open = false; };
    // scrolling or resizing moves the button away from the menu: close it (a scroll inside the menu itself does not count)
    const away = (e: Event) => { if (!inside(e.target)) open = false; };
    document.addEventListener("pointerdown", close);
    window.addEventListener("scroll", away, true);
    window.addEventListener("resize", away);
    return () => { document.removeEventListener("pointerdown", close); window.removeEventListener("scroll", away, true); window.removeEventListener("resize", away); };
  });
</script>

<div class="sel" bind:this={root}>
  <button class="trigger" bind:this={trigger} class:open aria-haspopup="listbox" aria-expanded={open} aria-label={label} onclick={toggle} onkeydown={onkey}>
    <span class="cur"><bdi>{current?.label}</bdi></span>
    {#if current?.hint}<span class="hint" class:warn={current.muted}>{current.hint}</span>{/if}
    <span class="chev" class:flip={open}><Icon name="chevron" size={14} /></span>
  </button>

  {#if open}
    <ul class="menu" role="listbox" aria-label={label} style={place} bind:this={menuEl} use:toBody>
      {#each options as o, i (o.value)}
        <li role="option" aria-selected={o.value === value} class:active={i === active} class:sel={o.value === value}
          onpointerenter={() => (active = i)} onclick={() => pick(o)} onkeydown={onkey} tabindex="-1">
          <span class="name" class:muted={o.muted}><bdi>{o.label}</bdi></span>
          {#if o.hint}<span class="hint" class:warn={o.muted}>{o.hint}</span>{/if}
          {#if o.value === value}<Icon name="check" size={14} />{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sel { position: relative; flex: 1 1 150px; min-width: 150px; }
  .trigger { width: 100%; display: flex; align-items: center; gap: 8px; padding-block: 8px; padding-inline: 14px 12px; border-radius: 999px; text-align: start;
    background: var(--surface-hi); border: 1px solid var(--stroke); transition: border-color .18s, background .18s; }
  .trigger:hover, .trigger.open { border-color: var(--stroke-hi); }
  .trigger.open { border-color: var(--accent); }
  .trigger:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .cur { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { flex: none; font-size: 11px; font-weight: 600; color: var(--ok); }
  .hint.warn { color: var(--warn); }
  .chev { display: grid; color: var(--text-faint); transition: transform .2s var(--ease); }
  .chev.flip { transform: rotate(180deg); }

  .menu { position: fixed; z-index: 40; width: max-content; max-width: min(340px, 86vw); margin: 0; padding: 6px; list-style: none;
    overflow-y: auto; border-radius: var(--r-md);
    background: var(--surface-solid); border: 1px solid var(--stroke-hi); box-shadow: var(--shadow);
    backdrop-filter: var(--blur); animation: drop .16s var(--ease); transform-origin: top; }
  li { display: flex; align-items: center; gap: 8px; padding: 9px 10px; border-radius: 10px; cursor: pointer; color: var(--text-dim); transition: background .12s; }
  li.active { background: var(--surface-hi); color: var(--text); }
  li.sel { color: var(--accent); font-weight: 600; }
  li .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  li .name.muted { opacity: .75; }
  @keyframes drop { from { opacity: 0; transform: translateY(-6px) scaleY(.96); } }
</style>
