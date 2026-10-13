<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";
  import Toggle from "./Toggle.svelte";

  let dialog: HTMLDivElement | undefined = $state();
  $effect(() => { if (app.settingsOpen) dialog?.focus(); });
</script>

<svelte:window onkeydown={(e) => { if (app.settingsOpen && e.key === "Escape") app.settingsOpen = false; }} />

{#if app.settingsOpen}
  <div class="scrim" role="presentation" onclick={() => (app.settingsOpen = false)}></div>
  <div class="glass dlg" role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1" bind:this={dialog}>
    <header>
      <h2>Settings</h2>
      <button class="btn" aria-label="Close" onclick={() => (app.settingsOpen = false)}><Icon name="x" size={16} /></button>
    </header>

    <div class="grid">
      <h3>Appearance</h3>
      <div class="line"><span>Theme</span>
        <div class="seg">
          <button class:on={app.theme === "dark"} onclick={() => app.setTheme("dark")}><Icon name="moon" size={14} /> Dark</button>
          <button class:on={app.theme === "light"} onclick={() => app.setTheme("light")}><Icon name="sun" size={14} /> Light</button>
        </div>
      </div>
      <div class="line"><span>Use mod cover as window background</span>
        <Toggle label="Cover as background" checked={app.coverBackdrop} onchange={(v) => app.setPref("coverBackdrop", v)} /></div>
      <div class="line"><span>Accent follows selected mod</span>
        <Toggle label="Accent follows mod" checked={app.accentFromMod} onchange={(v) => app.setPref("accentFromMod", v)} /></div>

      <h3>Game</h3>
      <div class="line"><span>Camera height</span>
        <div class="slider">
          <input type="range" min="0" max="500" step="10" value={app.cameraHeight}
            aria-label="Camera height" oninput={(e) => app.setPref("cameraHeight", +e.currentTarget.value)} />
          <output>{app.cameraHeight === 0 ? "Default" : app.cameraHeight}</output>
        </div>
      </div>
      <div class="line"><span>Run windowed</span><Toggle label="Windowed" checked={app.windowed} onchange={(v) => app.setPref("windowed", v)} /></div>
      <div class="line"><span>Skip intro (quick start)</span><Toggle label="Quick start" checked={app.quickStart} onchange={(v) => app.setPref("quickStart", v)} /></div>
    </div>
    <p class="note">Preview build: graphics options, GenTool, Vulkan and file-check settings arrive with the Rust core.</p>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: var(--scrim); backdrop-filter: blur(4px); z-index: 20; animation: fade .2s; }
  .dlg { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px));
    padding: 22px; background: var(--surface-solid); animation: pop .25s var(--ease); outline: none; max-height: calc(100vh - 32px); overflow: auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  h2 { font-size: 20px; }
  h3 { font-size: 11px; text-transform: uppercase; letter-spacing: .08em; color: var(--text-faint); margin: 18px 0 4px; }
  .line { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 0; border-bottom: 1px solid var(--stroke); }
  .seg { display: flex; padding: 3px; gap: 3px; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); }
  .seg button { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-radius: 999px; font-size: 13px; color: var(--text-dim); }
  .seg button.on { background: var(--accent); color: var(--accent-ink); font-weight: 600; }
  .slider { display: flex; align-items: center; gap: 12px; }
  input[type="range"] { accent-color: var(--accent); width: 170px; }
  output { min-width: 56px; text-align: right; color: var(--text-dim); font-variant-numeric: tabular-nums; }
  .note { margin-top: 16px; color: var(--text-faint); font-size: 12px; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -46%) scale(.97); } }
</style>
