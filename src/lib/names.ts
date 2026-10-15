import { t } from "./i18n/index.svelte";

/** The name to show for an item: the stock game has a built-in name that follows the language, mods keep their own. */
export const modName = (item: { id: string; name: string }): string => (item.id === "original-game" ? t("Original Game") : item.name);
