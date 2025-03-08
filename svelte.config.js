// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  // Custom preprocess options for handling PostCSS issues
  preprocess: {
    markup: ({ content }) => {
      return { code: content };
    },
    style: ({ content }) => {
      return { code: content };
    },
    script: ({ content }) => {
      return { code: content };
    }
  },
  kit: {
    adapter: adapter(),
  },
};

export default config;
