<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { isTauri, optionsLoad, optionsSave, type Graphics } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import Toggle from "./Toggle.svelte";

  const DEFAULTS: Graphics = {
    width: 1920, height: 1080, textureReduction: 0, maxParticleCount: 2500, shadowVolumes: false, shadowDecals: true,
    buildingOcclusion: true, showTrees: true, cloudMap: false, lightMap: true, softWaterEdge: true, heatEffects: false,
    extraAnimations: true, dynamicLod: true, alternateMouse: false,
  };

  let dialog: HTMLDivElement | undefined = $state();
  let g = $state<Graphics>({ ...DEFAULTS });
  let path = $state("");
  let exists = $state(true);
  let args = $state("");
  let hide = $state(false);
  let error = $state("");
  let busy = $state(false);
  let loading = $state(false);

  const screenSize = () => ({ width: Math.round(screen.width * devicePixelRatio), height: Math.round(screen.height * devicePixelRatio) });

  // (re)load whenever the dialog is opened
  $effect(() => {
    if (!app.optionsOpen) return;
    error = "";
    args = app.gameParams;
    hide = app.hideLauncher;
    queueMicrotask(() => dialog?.focus());
    if (!isTauri) { g = { ...DEFAULTS }; path = "C:\\Users\\you\\Documents\\Command and Conquer Generals Zero Hour Data\\Options.ini"; return; }
    loading = true;
    optionsLoad()
      .then((r) => { g = r.graphics; path = r.path; exists = r.exists; })
      .catch((e) => (error = e instanceof Error ? e.message : String(e)))
      .finally(() => (loading = false));
  });

  const PRESETS = [[800, 600], [1024, 768], [1280, 720], [1280, 1024], [1600, 900], [1920, 1080], [2560, 1440], [3440, 1440], [3840, 2160]];
  const resolutionOptions = $derived.by(() => {
    const list = PRESETS.map(([w, h]) => `${w} x ${h}`);
    const own = `${g.width} x ${g.height}`;
    if (!list.includes(own)) list.unshift(own);
    return list.map((v) => ({ value: v, label: v }));
  });
  const QUALITY: { v: number; label: string }[] = [{ v: 0, label: "High" }, { v: 1, label: "Medium" }, { v: 2, label: "Low" }];
  const TOGGLES: { key: keyof Graphics; label: string }[] = [
    { key: "shadowVolumes", label: "Shadow volumes" },
    { key: "shadowDecals", label: "Shadow decals" },
    { key: "buildingOcclusion", label: "Show units behind buildings" },
    { key: "showTrees", label: "Trees" },
    { key: "cloudMap", label: "Cloud shadows" },
    { key: "lightMap", label: "Light map" },
    { key: "softWaterEdge", label: "Soft water edges" },
    { key: "heatEffects", label: "Heat haze" },
    { key: "extraAnimations", label: "Extra animations" },
    { key: "dynamicLod", label: "Dynamic detail (LOD)" },
    { key: "alternateMouse", label: "Alternate mouse controls" },
  ];

  function pickResolution(v: string) {
    const [w, h] = v.split(" x ").map(Number);
    g.width = w; g.height = h;
  }
  function recommended() {
    const s = screenSize();
    g = { ...DEFAULTS, width: s.width, height: s.height };
  }

  async function save() {
    error = "";
    busy = true;
    try {
      if (isTauri) await optionsSave(g);
      app.setLaunchExtras(args.trim(), hide);
      app.say("Game options saved", "success");
      app.optionsOpen = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
  const close = () => { if (!busy) app.optionsOpen = false; };
</script>

<svelte:window onkeydown={(e) => { if (app.optionsOpen && e.key === "Escape") close(); }} />

{#if app.optionsOpen}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="glass dlg" role="dialog" aria-modal="true" aria-label="Game options" tabindex="-1" bind:this={dialog}>
    <header>
      <h2>Game options</h2>
      <button class="btn" aria-label="Close" onclick={close}><Icon name="x" size={16} /></button>
    </header>

    <div class="grid" aria-busy={loading}>
      <h3>Display</h3>
      <div class="line"><span>Resolution</span>
        <div class="pair">
          <Select label="Resolution" options={resolutionOptions} value={`${g.width} x ${g.height}`} onchange={pickResolution} />
          <button class="btn" title="Use the size of this screen" onclick={() => { const s = screenSize(); g.width = s.width; g.height = s.height; }}>This screen</button>
        </div>
      </div>
      <div class="line"><span>Texture quality</span>
        <div class="seg" role="group" aria-label="Texture quality">
          {#each QUALITY as q}<button class:on={g.textureReduction === q.v} onclick={() => (g.textureReduction = q.v)}>{q.label}</button>{/each}
        </div>
      </div>
      <div class="line"><span>Particle limit</span>
        <div class="slider">
          <input type="range" min="500" max="5000" step="100" value={Math.min(5000, Math.max(500, g.maxParticleCount))}
            aria-label="Particle limit" oninput={(e) => (g.maxParticleCount = +e.currentTarget.value)} />
          <output>{g.maxParticleCount}</output>
        </div>
      </div>

      <h3>Effects</h3>
      {#each TOGGLES as t}
        <div class="line"><span>{t.label}</span>
          <Toggle label={t.label} checked={g[t.key] as boolean} onchange={(v) => ((g as unknown as Record<string, unknown>)[t.key] = v)} /></div>
      {/each}

      <h3>Launching</h3>
      <div class="line"><span>Hide the launcher while the game runs</span><Toggle label="Hide launcher while playing" checked={hide} onchange={(v) => (hide = v)} /></div>
      <label class="line col"><span>Extra game arguments</span>
        <input bind:value={args} placeholder="e.g. -fps 60" spellcheck="false" maxlength="300" /></label>
    </div>

    <p class="note">
      These are the game's own settings{exists ? "" : " (the game has not created its file yet; it is created on save)"}, stored in<br />
      <code>{path}</code>. A copy of the original file is kept as <code>Options.ini.bak</code>.
    </p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <footer>
      <button class="btn" disabled={busy} onclick={recommended}><Icon name="undo" size={14} /> Recommended</button>
      <span class="spacer"></span>
      <button class="btn" disabled={busy} onclick={close}>Cancel</button>
      <button class="btn primary" disabled={busy || loading} onclick={save}>Save</button>
    </footer>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: var(--scrim); backdrop-filter: blur(4px); z-index: 20; animation: fade .2s; }
  .dlg { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(540px, calc(100vw - 32px));
    padding: 22px; background: var(--surface-solid); animation: pop .25s var(--ease); outline: none; max-height: calc(100vh - 32px); overflow: auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  h2 { font-size: 20px; }
  h3 { font-size: 11px; text-transform: uppercase; letter-spacing: .08em; color: var(--text-faint); margin: 18px 0 4px; }
  .line { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 9px 0; border-bottom: 1px solid var(--stroke); }
  .line.col { flex-direction: column; align-items: stretch; gap: 8px; }
  .line.col input { padding: 9px 14px; border-radius: 12px; background: var(--surface); border: 1px solid var(--stroke); outline: none; user-select: text; }
  .line.col input:focus { border-color: var(--accent); }
  .pair { display: flex; gap: 8px; align-items: center; }
  .seg { display: flex; padding: 3px; gap: 3px; border-radius: 999px; background: var(--surface); border: 1px solid var(--stroke); }
  .seg button { padding: 6px 14px; border-radius: 999px; font-size: 13px; color: var(--text-dim); }
  .seg button.on { background: var(--accent); color: var(--accent-ink); font-weight: 600; }
  .slider { display: flex; align-items: center; gap: 12px; }
  input[type="range"] { accent-color: var(--accent); width: 170px; }
  output { min-width: 48px; text-align: right; color: var(--text-dim); font-variant-numeric: tabular-nums; }
  .note { margin-top: 14px; color: var(--text-faint); font-size: 12px; overflow-wrap: anywhere; }
  .note code { background: var(--surface-hi); padding: 1px 6px; border-radius: 6px; user-select: text; }
  .error { color: var(--danger); margin-top: 10px; }
  footer { display: flex; align-items: center; gap: 10px; margin-top: 16px; }
  .spacer { flex: 1; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -46%) scale(.97); } }
</style>
