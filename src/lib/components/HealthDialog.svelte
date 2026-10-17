<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";
  import { describe, FIX_LABELS } from "$lib/health";
  import { t } from "$lib/i18n/index.svelte";

  let dialog: HTMLDivElement | undefined = $state();
  $effect(() => { if (app.healthOpen) { dialog?.focus(); void app.runHealthCheck(); } });
  const icon = { ok: "check", info: "bell", warn: "tool", bad: "x" } as const;
</script>

<svelte:window onkeydown={(e) => { if (app.healthOpen && e.key === "Escape") app.healthOpen = false; }} />

{#if app.healthOpen}
  <div class="scrim" role="presentation" onclick={() => (app.healthOpen = false)}></div>
  <div class="glass dlg" role="dialog" aria-modal="true" aria-label={t("Game health")} tabindex="-1" bind:this={dialog}>
    <header>
      <h2>{t("Game health")}</h2>
      <button class="btn" aria-label={t("Close")} onclick={() => (app.healthOpen = false)}><Icon name="x" size={16} /></button>
    </header>
    <p class="lead">{t("GenLauncher looks for the usual reasons a game will not start or mods look wrong. It only changes something when you press a button, and every change can be undone.")}</p>

    {#if app.healthBusy && !app.health.length}
      <p class="note">{t("Checking…")}</p>
    {/if}
    <ul>
      {#each app.health as f (f.id)}
        {@const w = describe(f)}
        <li class={f.level}>
          <span class="ico"><Icon name={icon[f.level]} size={16} /></span>
          <div class="txt">
            <strong>{w.title}</strong>
            <p>{w.detail}</p>
          </div>
          {#if f.fix}
            <button class="btn" disabled={app.healthBusy || app.launch !== "idle"} onclick={() => app.applyHealthFix(f.fix ?? "")}>{t(FIX_LABELS[f.fix] ?? f.fix)}</button>
          {/if}
        </li>
      {/each}
    </ul>
    <footer>
      <button class="btn" disabled={app.healthBusy} onclick={() => app.runHealthCheck()}><Icon name="refresh" size={14} /> {t("Check again")}</button>
    </footer>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: var(--scrim); backdrop-filter: blur(4px); z-index: 20; animation: fade .2s; }
  .dlg { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(600px, calc(100vw - 32px));
    padding: 22px; background: var(--surface-solid); animation: pop .25s var(--ease); outline: none; max-height: calc(100vh - 32px); overflow: auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
  h2 { font-size: 20px; }
  .lead, .note { color: var(--text-dim); font-size: 13px; margin: 0 0 14px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  li { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 14px; background: var(--surface); border: 1px solid var(--stroke); }
  li.warn { border-color: var(--warn); }
  li.bad { border-color: var(--danger); }
  .ico { flex: none; display: grid; place-items: center; width: 30px; height: 30px; border-radius: 50%; background: var(--surface-hi); color: var(--text-dim); }
  li.ok .ico { color: var(--ok); }
  li.warn .ico { color: var(--warn); }
  li.bad .ico { color: var(--danger); }
  .txt { flex: 1; min-width: 0; }
  .txt p { margin: 3px 0 0; color: var(--text-dim); font-size: 13px; overflow-wrap: anywhere; }
  footer { display: flex; justify-content: flex-end; margin-top: 14px; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -46%) scale(.97); } }
</style>
