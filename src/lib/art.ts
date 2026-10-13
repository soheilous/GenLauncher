import type { Item } from "./types";

// Generated placeholder art from a hue; used when a mod has no cover image.
export function art(hue: number): string {
  return (
    `radial-gradient(120% 90% at 20% 10%, hsl(${hue} 85% 58% / .95), transparent 60%),` +
    `radial-gradient(90% 80% at 90% 90%, hsl(${(hue + 50) % 360} 80% 40% / .9), transparent 65%),` +
    `linear-gradient(135deg, hsl(${hue} 55% 18%), hsl(${(hue + 30) % 360} 60% 10%))`
  );
}

/** CSS `background` value for a card or hero: the cover image if present, else generated art. */
export function coverStyle(item: Pick<Item, "cover" | "hue">): string {
  return item.cover ? `url("${item.cover}") center / cover no-repeat, ${art(item.hue)}` : art(item.hue);
}
