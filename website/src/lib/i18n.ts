// Translations for the website. The code keeps the English text as the key: t("Features"). A language file
// (src/lib/locales/<code>.ts) maps English to its language; anything it lacks falls back to English.
//
//   t("Features")                              plain text
//   t(text, { name: value })                   values are put in at {name}
//   tn(n, "{n} mod", "{n} mods")               plural: English forms, a language may give its own under the "other" text
//   N("Safe by design")                        marks a text for translation where it is only stored (shown later with t)
//
// Texts with a link, bold, italic or code use a few markdown marks: [label](/guide), **bold**, *italic*, `code`.
// They are shown with <Rich text={t(text)} />. Addresses starting with "/" get the page's language prefix.
// tools/check-i18n.mjs --site lists what a language still lacks.

import { getContext, setContext } from "svelte";

export type Plural = Partial<Record<Intl.LDMLPluralRule, string>>;
export type Messages = Record<string, string | Plural>;
type Vars = Record<string, string | number>;

export interface Language {
  code: string;
  /** The language's own name. */
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

/** The codes that appear in addresses (English is the unprefixed default). */
export const OTHER_LANGUAGES = LANGUAGES.filter((l) => l.code !== "en").map((l) => l.code);

export const N = (text: string): string => text;

export const dirOf = (lang: string): "ltr" | "rtl" => LANGUAGES.find((l) => l.code === lang)?.dir ?? "ltr";

/** "/mods" -> "/fa/mods"; "/#try" -> "/fa#try"; English keeps the plain address. */
export function localize(lang: string, path: string): string {
  const [p, hash] = path.split("#");
  const base = lang === "en" ? p || "/" : "/" + lang + (p === "/" || p === "" ? "" : p);
  return base + (hash ? "#" + hash : "");
}

/** The address of the same page in another language. */
export function switchLanguage(pathname: string, lang: string): string {
  const rest = pathname.replace(new RegExp(`^/(${OTHER_LANGUAGES.join("|")})(?=/|$)`), "") || "/";
  return localize(lang, rest);
}

function fill(text: string, vars: Vars | undefined, isolate: boolean): string {
  if (!vars) return text;
  // in a right-to-left language a value (a number, a version) is kept in its own direction
  return text.replace(/\{(\w+)\}/g, (whole, name) => (name in vars ? (isolate ? `⁨${vars[name]}⁩` : String(vars[name])) : whole));
}

export interface I18n {
  readonly lang: string;
  readonly dir: "ltr" | "rtl";
  t(text: string, vars?: Vars): string;
  tn(n: number, one: string, other: string, vars?: Vars): string;
  /** A site address in the page's language. */
  href(path: string): string;
}

const KEY = Symbol("i18n");

/** Called once by the layout; everything below it reads the language through getI18n(). */
export function provideI18n(get: () => { lang: string; messages: Messages }): I18n {
  const api: I18n = {
    get lang() { return get().lang; },
    get dir() { return dirOf(get().lang); },
    t(text, vars) {
      const { lang, messages } = get();
      const m = messages[text];
      return fill(typeof m === "string" ? m : text, vars, dirOf(lang) === "rtl");
    },
    tn(n, one, other, vars) {
      const { lang, messages } = get();
      const all: Vars = { ...vars, n };
      const isolate = dirOf(lang) === "rtl";
      const m = messages[other];
      if (m && typeof m === "object") return fill(m[new Intl.PluralRules(lang).select(n)] ?? m.other ?? other, all, isolate);
      if (typeof m === "string") return fill(m, all, isolate);
      return fill(n === 1 ? one : other, all, isolate);
    },
    href: (path) => localize(get().lang, path),
  };
  setContext(KEY, api);
  return api;
}

export const getI18n = (): I18n => getContext<I18n>(KEY);
