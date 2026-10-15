import type { RequestHandler } from "./$types";
import { LANGUAGES, localize } from "$lib/i18n";
import { siteOrigin } from "$lib/server/site";
import { PAGES } from "$lib/pages";

export const prerender = true;

/** Every page in every language, each listing its translations (https://developers.google.com/search/docs/specialty/international/localized-versions). */
export const GET: RequestHandler = ({ url }) => {
  const origin = siteOrigin() || url.origin;
  const loc = (lang: string, path: string) => origin + localize(lang, path);
  const entries = PAGES.flatMap((path) =>
    LANGUAGES.map((l) => {
      const alts = [...LANGUAGES.map((a) => `<xhtml:link rel="alternate" hreflang="${a.code}" href="${loc(a.code, path)}"/>`), `<xhtml:link rel="alternate" hreflang="x-default" href="${loc("en", path)}"/>`].join("");
      return `<url><loc>${loc(l.code, path)}</loc>${alts}</url>`;
    }),
  );
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${entries.join("")}</urlset>\n`;
  return new Response(body, { headers: { "content-type": "application/xml" } });
};
