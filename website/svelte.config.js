import adapter from "@sveltejs/adapter-auto";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  // every page is prerendered; adapter-auto picks the right output on Vercel
  kit: { adapter: adapter() },
};
