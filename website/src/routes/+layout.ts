import type { LayoutLoad } from "./$types";

// the whole site is static pages, one set per language
export const prerender = true;

/** The page's language and its messages. Only that language's file is loaded, so a page ships one translation. */
export const load: LayoutLoad = async ({ params, data }) => {
  const lang = params.lang ?? "en";
  const messages = lang === "en" ? {} : (await import(`../lib/locales/${lang}.ts`)).default;
  return { ...data, lang, messages };
};
