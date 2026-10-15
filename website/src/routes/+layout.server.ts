import type { LayoutServerLoad } from "./$types";
import { siteOrigin } from "$lib/server/site";

export const load: LayoutServerLoad = () => ({ origin: siteOrigin() });
