import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri ships a single static bundle; SPA fallback enables client-side routing.
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
      strict: true
    }),
    alias: {
      $components: 'src/lib/components',
      $stores: 'src/lib/stores',
      $tauri: 'src/lib/tauri',
      $types: 'src/lib/types.ts',
      $utils: 'src/lib/utils'
    }
  }
};

export default config;
