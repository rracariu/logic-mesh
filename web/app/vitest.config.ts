import { defineConfig } from 'vitest/config';

// The unit tests cover plain modules — no Svelte components, no `$lib`
// imports — so they run without vite.config.ts's SvelteKit plugin.
export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
  },
});
