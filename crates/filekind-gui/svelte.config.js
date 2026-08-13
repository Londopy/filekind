import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri serves a bundled directory, not a Node server. Everything is
    // prerendered to static files and loaded from disk.
    adapter: adapter({ fallback: 'index.html', strict: false })
  }
};
