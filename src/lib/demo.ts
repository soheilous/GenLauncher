// The interactive demo on the website: this same interface, built with sample data and no access to the computer.
// The website page drives it with small messages ("go to Discover", "open the settings"); nothing here exists in the
// real app.

import { app } from "./app.svelte";
import { isDemo } from "./env";
import { i18n, t } from "./i18n/index.svelte";

type Command =
  | { type: "theme"; value: "dark" | "light" }
  | { type: "language"; value: string }
  | { type: "view"; value: "library" | "discover" | "downloads" | "notifications" }
  | { type: "open"; value: "settings" | "options" | "add" | "drop" }
  | { type: "select"; value: string }
  | { type: "install" }
  | { type: "play" }
  | { type: "close" };

const SAMPLE_FILE = "C:\\Downloads\\Hotfix_v2.1.zip";

function closeEverything() {
  app.settingsOpen = false;
  app.optionsOpen = false;
  app.closeAdd();
}

function run(c: Command) {
  switch (c.type) {
    case "theme": app.setTheme(c.value); break;
    case "language": i18n.choose(c.value); break;
    case "view": closeEverything(); app.view = c.value; break;
    case "select": closeEverything(); app.view = "library"; void app.selectMod(c.value); break;
    case "close": closeEverything(); break;
    case "open":
      closeEverything();
      // dialogs open over the library (the screen the visitor knows), whatever was showing before
      if (c.value !== "settings") app.view = "library";
      if (c.value === "settings") app.settingsOpen = true;
      else if (c.value === "options") app.optionsOpen = true;
      else if (c.value === "add") app.openAdd("mod");
      else app.openAdd("mod", null, { name: "", pageUrl: null, update: false, file: SAMPLE_FILE, askType: true });
      break;
    case "install": {
      // a mod that is not installed yet, installed the way the real app shows it: with progress, in Discover
      closeEverything();
      app.view = "discover";
      const item = app.availableMods[0];
      if (item) void app.install(item);
      break;
    }
    case "play":
      closeEverything();
      app.view = "library";
      void app.play("game");
      break;
  }
}

/** A few notifications, as a used launcher would have them. */
function seedNotifications() {
  app.clearNotices();
  const now = Date.now();
  app.notify(t("Game closed after {time}. Game folder restored.", { time: t("{n} min", { n: 42 }) }), "info", { id: "demo-1", at: now - 3_600_000 });
  app.notify(t("{name}: {error}", { name: "Shockwave", error: t("installation failed") }), "error", { id: "demo-2", at: now - 86_400_000 });
  app.notify(t("{name} {version} installed", { name: "Contra", version: "10.0.2" }), "success", { id: "demo-3", at: now - 90_000 });
}

export function initDemo() {
  if (!isDemo || typeof window === "undefined") return;
  const q = new URLSearchParams(location.search);
  const theme = q.get("theme");
  if (theme === "dark" || theme === "light") app.setTheme(theme);
  const lang = q.get("lang");
  if (lang) i18n.choose(lang);
  seedNotifications();

  window.addEventListener("message", (e: MessageEvent) => {
    // only the page that embeds us, on our own origin
    if (e.origin !== location.origin || e.source !== window.parent) return;
    const m = e.data as { source?: string; command?: Command };
    if (m?.source === "genlauncher-site" && m.command) run(m.command);
  });
  if (window.parent !== window) window.parent.postMessage({ source: "genlauncher-demo", type: "ready" }, location.origin);
}
