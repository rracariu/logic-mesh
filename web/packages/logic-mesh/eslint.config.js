import js from '@eslint/js';
import ts from 'typescript-eslint';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default ts.config(
  js.configs.recommended,
  ...ts.configs.recommended,
  prettier,
  {
    languageOptions: {
      globals: globals.browser,
      // typescript-eslint v8+ needs `tsconfigRootDir` to be explicit
      // when more than one workspace under the same git tree has a
      // `tsconfig.json` (we have web/app + web/packages/logic-mesh).
      // Pinning to this config file's own directory makes the IDE pick
      // the correct workspace per file.
      parserOptions: {
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    rules: {
      // Intentionally-unused parameters/vars use a `_` prefix; that
      // convention is widely used here, so don't flag them.
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
      // Hard errors. Test-side `any` is allowed via a file-level
      // disable in `test/TypedBlock.test.ts` (negative-path tests
      // need wrong-shape inputs).
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-empty-object-type': 'error',
    },
  },
  {
    // `dist/` and `pkg/` are wasm-pack output; never our source.
    ignores: ['dist/', 'pkg/', 'node_modules/'],
  },
);
