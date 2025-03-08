import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    // Custom plugin to bypass PostCSS errors
    {
      name: 'skip-postcss-svelte',
      transform(code, id) {
        if (id.includes('.svelte') && id.includes('type=style')) {
          // Skip CSS transforms for Svelte style tags
          return { code };
        }
      }
    },
    // Regular SvelteKit plugin
    sveltekit(),
  ],

  // Disable built-in CSS processing to avoid PostCSS errors
  css: { 
    transformer: 'postcss',
    postcss: { plugins: [] } 
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
