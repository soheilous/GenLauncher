<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { modName } from "$lib/names";
  import Toggle from "./Toggle.svelte";

  const picked = $derived(app.children.filter((c) => c.selected));
  const label = $derived(app.launch === "idle" ? t("Play") : app.launch === "preparing" ? t("Preparing files…") : t("Game running"));
</script>

<footer class="glass bar">
  <div class="summary">
    <strong><bdi>{modName(app.activeMod)}</bdi></strong>
    <div class="chips">
      {#each picked as p}<span class="chip accent"><bdi>{p.name}</bdi></span>{:else}<span class="chip">{t("Base mod only")}</span>{/each}
    </div>
  </div>

  <div class="opt"><Toggle label={t("Windowed")} checked={app.windowed} onchange={(v) => app.setPref("windowed", v)} /> {t("Windowed")}</div>
  <div class="opt"><Toggle label={t("Quick start")} checked={app.quickStart} onchange={(v) => app.setPref("quickStart", v)} /> {t("Quick start")}</div>

  <button class="btn" disabled={app.launch !== "idle"} onclick={() => app.play("worldbuilder")}><Icon name="tool" size={16} /> {t("World Builder")}</button>
  <button class="btn primary play" disabled={app.launch !== "idle"} onclick={() => app.play("game")}>
    <Icon name="play" size={18} /> {label}
  </button>
</footer>

<style>
  .bar { display: flex; align-items: center; gap: 18px; padding: 14px 18px 14px 22px; }
  .summary { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .opt { display: flex; align-items: center; gap: 8px; color: var(--text-dim); font-size: 13px; }
  .play { padding: 13px 30px; font-size: 16px; min-width: 190px; justify-content: center; box-shadow: 0 8px 28px hsl(var(--accent-h) 85% 55% / .45); }
</style>
