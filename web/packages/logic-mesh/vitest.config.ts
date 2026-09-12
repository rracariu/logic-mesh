import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    // Tests import the package sources (`../src`), whose runtime
    // imports of `./logic_mesh.js` only resolve next to the compiled
    // output — wasm-pack emits that module into `dist/`. Point the
    // specifier at the built module so the suite runs against the real
    // wasm engine in Node (build first: `npm run build:dev`).
    alias: [
      {
        find: /^\.\/logic_mesh\.js$/,
        replacement: fileURLToPath(
          new URL('./dist/logic_mesh.js', import.meta.url),
        ),
      },
    ],
  },
  test: {
    include: ['test/**/*.test.ts'],
  },
});
