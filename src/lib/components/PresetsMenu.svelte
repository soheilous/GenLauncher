<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { isTauri } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { modName } from "$lib/names";
  import type { Preset } from "$lib/types";

  let open = $state(false);
  let name = $state("");
  let root: HTMLDivElement | undefined = $state();
  const list = $derived(app.presets.filter((p) => p.game === app.game));

  $effect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => { if (root && !root.contains(e.target as Node)) open = false; };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  });

  function save() {
    const p = app.addPreset(name);
    if (p) name = "";
  }

  async function copyCommand(p: Preset) {
    const cmd = await app.commandFor(p);
    if (!cmd) return;
    try { await navigator.clipboard.writeText(cmd); app.say(t("Copied. In Steam, Add a non-Steam game, then paste it into Launch options."), "success"); }
    catch { app.say(cmd); }
  }

  const modOf = (p: Preset) => { const m = app.mods.find((x) => x.id === p.mod); return m ? modName(m) : p.mod; };
</script>

<div class="presets" bind:this={root} onkeydown={(e) => { if (e.key === "Escape") open = false; }} role="presentation">
  <button class="btn" aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = !open)}><Icon name="star" size={16} /> {t("Presets")}</button>
  {#if open}
    <div class="menu glass" role="dialog" aria-label={t("Presets")}>
      <p class="hint">{t("A preset remembers the mod, its version, the patches and add-ons you switched on, and the windowed / quick start options.")}</p>
      {#each list as p (p.id)}
        <div class="item">
          <div class="who"><strong><bdi>{p.name}</bdi></strong><small><bdi>{modOf(p)}</bdi>{#if p.enabled.length} · {p.enabled.length}{/if}</small></div>
          <button class="btn primary" title={t("Play this preset")} aria-label={t("Play this preset")} disabled={app.launch !== "idle"} onclick={() => { open = false; void app.playPreset(p.id); }}><Icon name="play" size={14} /></button>
          <button class="btn" title={t("Put its choices on screen")} aria-label={t("Put its choices on screen")} onclick={async () => { await app.applyPreset(p); open = false; }}><Icon name="check" size={14} /></button>
          {#if isTauri}
            <button class="btn" title={t("Make a desktop shortcut")} aria-label={t("Make a desktop shortcut")} onclick={() => app.makeShortcut(p)}><Icon name="shortcut" size={14} /></button>
            <button class="btn" title={t("Copy the command for Steam")} aria-label={t("Copy the command for Steam")} onclick={() => copyCommand(p)}><Icon name="copy" size={14} /></button>
          {/if}
          <button class="btn" title={t("Delete this preset")} aria-label={t("Delete this preset")} onclick={() => app.removePreset(p.id)}><Icon name="trash" size={14} /></button>
        </div>
      {:else}
        <p class="empty">{t("No presets yet for this game.")}</p>
      {/each}
      <form class="add" onsubmit={(e) => { e.preventDefault(); save(); }}>
        <input placeholder={t("Name for the current choices")} aria-label={t("Name for the current choices")} bind:value={name} maxlength="60" />
        <button class="btn primary" type="submit" disabled={!name.trim()}><Icon name="plus" size={14} /> {t("Save")}</button>
      </form>
    </div>
  {/if}
</div>

<style>
  .presets { position: relative; }
  .menu { position: absolute; bottom: calc(100% + 12px); inset-inline-end: 0; width: min(420px, 90vw); padding: 14px; display: grid; gap: 8px; z-index: 12; max-height: 60vh; overflow: auto; }
  .hint, .empty { margin: 0; color: var(--text-faint); font-size: 12px; }
  .item { display: flex; align-items: center; gap: 6px; padding: 8px 10px; border-radius: 12px; background: var(--surface); border: 1px solid var(--stroke); }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .who small { color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item .btn { padding: 7px; }
  .add { display: flex; gap: 8px; margin-top: 4px; }
  .add input { flex: 1; min-width: 0; padding: 9px 14px; border-radius: 999px; border: 1px solid var(--stroke); background: var(--surface); color: var(--text); font: inherit; outline: none; }
  .add input:focus { border-color: var(--accent); }
</style>
