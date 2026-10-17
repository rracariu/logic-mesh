// The exact file list of the published tarball, shared by the
// `prepublishOnly` guard (check-dist.mjs) and the pack smoke test
// (test-pack.mjs): a file missing from dist/, or one left over from an
// older build, fails both before it can ship.
import { existsSync, readdirSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The package root (web/packages/logic-mesh). */
export const packageDir = fileURLToPath(new URL('..', import.meta.url));

/** The repository root, home of the LICENSE the build copies in. */
export const repoDir = join(packageDir, '..', '..', '..');

/** The wasm-pack `bundler` glue that `build:prepare` moves into dist/. */
const WASM_GLUE = [
  'logic_mesh.d.ts',
  'logic_mesh.js',
  'logic_mesh_bg.js',
  'logic_mesh_bg.wasm',
  'logic_mesh_bg.wasm.d.ts',
];

/**
 * Returns the sorted paths, relative to the package root, that
 * `npm pack` must produce: the manifest, README and LICENSE, the wasm
 * glue, and the `tsc` output (a `.js` and a `.d.ts`) of every source
 * module in src/.
 */
export function expectedPackageFiles() {
  const compiled = readdirSync(join(packageDir, 'src'))
    .filter((name) => name.endsWith('.ts') && !name.endsWith('.d.ts'))
    .flatMap((name) => {
      const stem = name.slice(0, -'.ts'.length);
      return [`dist/${stem}.d.ts`, `dist/${stem}.js`];
    });
  return [
    'LICENSE',
    'README.md',
    'package.json',
    ...WASM_GLUE.map((name) => `dist/${name}`),
    ...compiled,
  ].sort();
}

/**
 * Compares the build output on disk with {@link expectedPackageFiles}.
 *
 * @returns The expected files that are absent, and the files under
 * dist/ that no build step produces (stale or hand-copied).
 */
export function checkBuildOutput() {
  const expected = expectedPackageFiles();
  const missing = expected.filter(
    (path) => !existsSync(join(packageDir, path)),
  );
  const distDir = join(packageDir, 'dist');
  const present = existsSync(distDir)
    ? readdirSync(distDir, { recursive: true, withFileTypes: true })
        .filter((entry) => !entry.isDirectory())
        .map((entry) =>
          relative(packageDir, join(entry.parentPath, entry.name))
            .split(sep)
            .join('/'),
        )
    : [];
  const unexpected = present.filter((path) => !expected.includes(path)).sort();
  return { missing, unexpected };
}
