import adapter from "@sveltejs/adapter-auto";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  kit: {
    // every page is prerendered; adapter-auto picks the right output on Vercel
    adapter: adapter(),
    prerender: {
      // /demo/ is the launcher's interface built into static/demo (tools/build-demo.mjs): a static folder, not a route
      handleHttpError: ({ path, message }) => {
        if (path.startsWith("/demo")) return;
        throw new Error(message);
      },
    },
  },
};
