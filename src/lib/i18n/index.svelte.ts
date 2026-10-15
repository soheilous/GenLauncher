// Translations. The code keeps the English text as the key: `t("Install")`. A language file maps English to its own
// text; anything it does not have falls back to the English original, so a missing translation is never a blank.
//
//   t("Settings")                                    plain text
//   t("Adding {version}", { version })               values are put in at {name}
//   tn(n, "{n} backup", "{n} backups")               plural: the English forms; a language file may give its own
//                                                    forms (one / few / many / other...) under the "other" text
//
// tools/check-i18n.mjs lists what a language still lacks.

import { storeKey } from "../env";
import ar from "./locales/ar";
import de from "./locales/de";
import es from "./locales/es";
import fa from "./locales/fa";
import fr from "./locales/fr";
import it from "./locales/it";
import pl from "./locales/pl";
import pt from "./locales/pt";
import ru from "./locales/ru";
import tr from "./locales/tr";
import uk from "./locales/uk";
import zh from "./locales/zh";

export type Plural = Partial<Record<Intl.LDMLPluralRule, string>>;
export type Messages = Record<string, string | Plural>;
type Vars = Record<string, string | number>;

export interface Language {
  code: string;
  /** The language's own name, so people can find it whatever the current language is. */
  name: string;
  dir: "ltr" | "rtl";
}

export const LANGUAGES: Language[] = [
  { code: "en", name: "English", dir: "ltr" },
  { code: "fa", name: "فارسی", dir: "rtl" },
  { code: "fr", name: "Français", dir: "ltr" },
  { code: "es", name: "Español", dir: "ltr" },
  { code: "pt", name: "Português", dir: "ltr" },
  { code: "it", name: "Italiano", dir: "ltr" },
  { code: "de", name: "Deutsch", dir: "ltr" },
  { code: "zh", name: "简体中文", dir: "ltr" },
  { code: "ru", name: "Русский", dir: "ltr" },
  { code: "uk", name: "Українська", dir: "ltr" },
  { code: "pl", name: "Polski", dir: "ltr" },
  { code: "tr", name: "Türkçe", dir: "ltr" },
  { code: "ar", name: "العربية", dir: "rtl" },
];

/** In development only: "qps" wraps every text, so anything that is not translated-ready stands out on screen. */
const PSEUDO = "qps";
const pseudo = new Proxy({} as Messages, { get: (_, key) => (typeof key === "string" ? `⟦${key}⟧` : undefined) });

const bundles: Record<string, Messages> = { ar, de, es, fa, fr, it, pl, pt, ru, tr, uk, zh };
if (import.meta.env.DEV) bundles[PSEUDO] = pseudo;

const SUPPORTED = new Set(["en", ...Object.keys(bundles)]);
const AUTO = "auto";

function stored(): string {
  try { return localStorage.getItem(storeKey("language")) ?? AUTO; } catch { return AUTO; }
}

/** The best supported language for what the system asks for ("fa-IR" -> "fa"), English if none. */
function fromSystem(): string {
  const asked = typeof navigator === "undefined" ? [] : (navigator.languages?.length ? navigator.languages : [navigator.language]);
  for (const l of asked) {
    const base = (l ?? "").toLowerCase().split("-")[0];
    if (SUPPORTED.has(base) && base !== PSEUDO) return base;
  }
  return "en";
}

class I18n {
  /** What the person chose: a language code, or "auto" to follow the system. */
  setting = $state(stored());
  locale = $derived(this.setting !== AUTO && SUPPORTED.has(this.setting) ? this.setting : fromSystem());
  dir = $derived<"ltr" | "rtl">(LANGUAGES.find((l) => l.code === this.locale)?.dir ?? "ltr");
  private messages = $derived<Messages>(bundles[this.locale] ?? {});
  private rules = $derived(new Intl.PluralRules(this.locale === PSEUDO ? "en" : this.locale));

  choose(code: string) {
    this.setting = code === AUTO || SUPPORTED.has(code) ? code : AUTO;
    try { localStorage.setItem(storeKey("language"), this.setting); } catch { /* private window */ }
  }

  /** Mirror the language on the page itself (screen readers, hyphenation, right-to-left layout). */
  apply() {
    if (typeof document === "undefined") return;
    document.documentElement.lang = this.locale === PSEUDO ? "en" : this.locale;
    document.documentElement.dir = this.dir;
  }

  t(text: string, vars?: Vars): string {
    const m = this.messages[text];
    return fill(typeof m === "string" ? m : text, vars, this.dir === "rtl");
  }

  tn(n: number, one: string, other: string, vars?: Vars): string {
    const all: Vars = { ...vars, n };
    const m = this.messages[other];
    if (m && typeof m === "object") {
      return fill(m[this.rules.select(n)] ?? m.other ?? other, all, this.dir === "rtl");
    }
    if (typeof m === "string") return fill(m, all, this.dir === "rtl");
    return fill(this.locale === PSEUDO ? other : n === 1 ? one : other, all, this.dir === "rtl");
  }

  /** A date and time in the language's own format. */
  dateTime(ms: number, style: "short" | "long" = "short"): string {
    return new Date(ms).toLocaleString(this.locale === PSEUDO ? "en" : this.locale, style === "short" ? { dateStyle: "medium", timeStyle: "short" } : { dateStyle: "long", timeStyle: "short" });
  }

  date(ms: number): string {
    return new Date(ms).toLocaleDateString(this.locale === PSEUDO ? "en" : this.locale, { day: "numeric", month: "short", year: "numeric" });
  }
}

/** Puts the values in. In a right-to-left language each value is isolated, so "009 Beta 3" is not reordered by the sentence around it. */
function fill(text: string, vars?: Vars, isolate = false): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name) => (name in vars ? (isolate ? `\u2068${vars[name]}\u2069` : String(vars[name])) : whole));
}

export const i18n = new I18n();
export const t = (text: string, vars?: Vars) => i18n.t(text, vars);
export const tn = (n: number, one: string, other: string, vars?: Vars) => i18n.tn(n, one, other, vars);

/** Languages offered in the settings (the pseudo language only while developing). */
export const offered = (): Language[] => (import.meta.env.DEV ? [...LANGUAGES, { code: PSEUDO, name: "Pseudo (dev)", dir: "ltr" }] : LANGUAGES);

/** Marks a text for translation without translating it yet (labels kept in constants, shown with `t(label)`). */
export const N = (text: string) => text;
