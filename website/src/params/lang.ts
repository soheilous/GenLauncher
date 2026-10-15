import type { ParamMatcher } from "@sveltejs/kit";
import { OTHER_LANGUAGES } from "$lib/i18n";

/** The language segment of an address (/fa/mods). English has none. */
export const match: ParamMatcher = (param) => OTHER_LANGUAGES.includes(param);
