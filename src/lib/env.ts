/** True in the build that is embedded in the website as an interactive demo (made with `npm run build:demo`). */
export const isDemo = import.meta.env.VITE_DEMO === "1";

/** The demo runs on the website's own origin, so it keeps its settings under names of its own. */
export const storeKey = (key: string): string => (isDemo ? `demo:${key}` : key);
