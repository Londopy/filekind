import { sveltekit } from '@sveltejs/kit/vite';

const host = process.env.TAURI_DEV_HOST;

export default {
  plugins: [sveltekit()],
  // Tauri drives the dev server; failing loudly beats silently serving on
  // another port that the webview will not be pointed at.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 5183 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] }
  }
};
