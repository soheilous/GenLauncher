import type { Handle } from "@sveltejs/kit";
import { dirOf, OTHER_LANGUAGES } from "$lib/i18n";

/** Puts the page's language and reading direction on <html> (app.html has %lang% and %dir%). */
export const handle: Handle = async ({ event, resolve }) => {
  const first = event.url.pathname.split("/")[1];
  const lang = OTHER_LANGUAGES.includes(first) ? first : "en";
  return resolve(event, {
    transformPageChunk: ({ html }) => html.replace("%lang%", lang).replace("%dir%", dirOf(lang)),
  });
};
