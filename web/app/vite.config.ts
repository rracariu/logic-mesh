import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
  plugins: [
    tailwindcss(),
    wasm(),
    sveltekit({
      preprocess: vitePreprocess(),
      adapter: adapter({ fallback: 'index.html' }),
      paths: {
        base: process.env.NODE_ENV === 'production' ? '/logic-mesh' : '',
      },
    }),
  ],
  build: {
    target: 'esnext',
  },
  server: {
    fs: {
      allow: ['module'],
    },
  },
});
