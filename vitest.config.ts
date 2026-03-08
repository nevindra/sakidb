import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    alias: {
      '$lib': '/src/lib',
      '$app': '/src/app'
    },
    setupFiles: ['src/lib/stores/__tests__/setup.ts']
  },
  resolve: {
    alias: {
      '$lib': '/src/lib',
      '$app': '/src/app'
    }
  }
});
