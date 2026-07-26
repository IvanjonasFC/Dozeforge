import { defineConfig } from 'vitest/config';
import path from 'node:path';

// Standalone config for pure-logic unit tests (parsers). Deliberately does NOT
// load the SvelteKit Vite plugin so tests run fast in a plain Node environment.
export default defineConfig({
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib'),
    },
  },
  test: {
    include: ['src/lib/parsers/**/*.test.ts'],
    environment: 'node',
  },
});
