// The exact file list of the published tarball, and the build steps
// that produce it, shared by the `prepublishOnly` guard
// (check-dist.mjs) and the pack smoke test (test-pack.mjs): a file
// missing from dist/, one left over from an older build, or a source
// edited after the build fails both before it can ship.
import { existsSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The package root (web/packages/logic-mesh). */
export const packageDir = fileURLToPath(new URL('..', import.meta.url));

/** The repository root, home of the LICENSE the build copies in. */
export const repoDir = join(packageDir, '..', '..', '..');

/** The roots of this checkout, which every check defaults to. */
const checkout = { packageDir, repoDir };

/** The wasm-pack `bundler` glue that `build:prepare` moves into dist/. */
const WASM_GLUE = [
  'logic_mesh.d.ts',
  'logic_mesh.js',
  'logic_mesh_bg.js',
  'logic_mesh_bg.wasm',
  'logic_mesh_bg.wasm.d.ts',
];

/**
 * The manifests and configuration wasm-pack builds the crate with,
 * relative to the repository root.
 */
const RUST_CONFIG = [
  '.cargo/config.toml',
  'Cargo.lock',
  'Cargo.toml',
  'build.rs',
  'rust-toolchain.toml',
  'block_macro/Cargo.toml',
];

/** The directories, relative to the repository root, of the crate sources. */
const RUST_SOURCE_DIRS = ['src', 'block_macro/src'];

/** Converts a relative path to the `/`-separated form npm reports. */
function toPosix(path) {
  return path.split(sep).join('/');
}

/** Returns the `.rs` files under `dir`, recursively; none if it is absent. */
function rustSources(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
    .map((entry) => join(entry.parentPath, entry.name));
}

/** Returns the stems of the TypeScript modules in src/ that `tsc` compiles. */
function sourceModules(dirs) {
  return readdirSync(join(dirs.packageDir, 'src'))
    .filter((name) => name.endsWith('.ts') && !name.endsWith('.d.ts'))
    .map((name) => name.slice(0, -'.ts'.length));
}

/**
 * Returns the build steps that write the packed files, in build
 * order, each with the absolute paths of the files it reads and of
 * the files it writes: wasm-pack compiles the crate into the glue,
 * `build:prepare` copies the root LICENSE, and `tsc` compiles src/.
 */
function buildSteps(dirs) {
  const dist = (name) => join(dirs.packageDir, 'dist', name);
  const modules = sourceModules(dirs);
  return [
    {
      inputs: [
        ...RUST_CONFIG.map((path) => join(dirs.repoDir, path)),
        ...RUST_SOURCE_DIRS.flatMap((dir) =>
          rustSources(join(dirs.repoDir, dir)),
        ),
      ],
      outputs: WASM_GLUE.map(dist),
    },
    {
      inputs: [join(dirs.repoDir, 'LICENSE')],
      outputs: [join(dirs.packageDir, 'LICENSE')],
    },
    {
      inputs: [
        join(dirs.packageDir, 'tsconfig.json'),
        ...modules.map((stem) => join(dirs.packageDir, 'src', `${stem}.ts`)),
      ],
      outputs: modules.flatMap((stem) => [
        dist(`${stem}.d.ts`),
        dist(`${stem}.js`),
      ]),
    },
  ];
}

/**
 * Returns the sorted paths, relative to the package root, that
 * `npm pack` must produce: the manifest, README and LICENSE, the wasm
 * glue, and the `tsc` output (a `.js` and a `.d.ts`) of every source
 * module in src/.
 */
export function expectedPackageFiles(dirs = checkout) {
  const compiled = sourceModules(dirs).flatMap((stem) => [
    `dist/${stem}.d.ts`,
    `dist/${stem}.js`,
  ]);
  return [
    'LICENSE',
    'README.md',
    'package.json',
    ...WASM_GLUE.map((name) => `dist/${name}`),
    ...compiled,
  ].sort();
}

/**
 * Compares the build output on disk with {@link expectedPackageFiles}
 * and with the sources it was built from.
 *
 * @param dirs The package and repository roots; this checkout's by
 * default.
 * @returns `missing`: the expected files that are absent; `unexpected`:
 * the files under dist/ that no build step produces (stale or
 * hand-copied); `stale`: the build inputs, relative to the repository
 * root, modified after the step that reads them wrote its output, so
 * the output predates them.
 */
export function checkBuildOutput(dirs = checkout) {
  const expected = expectedPackageFiles(dirs);
  const missing = expected.filter(
    (path) => !existsSync(join(dirs.packageDir, path)),
  );
  const distDir = join(dirs.packageDir, 'dist');
  const present = existsSync(distDir)
    ? readdirSync(distDir, { recursive: true, withFileTypes: true })
        .filter((entry) => !entry.isDirectory())
        .map((entry) =>
          toPosix(
            relative(dirs.packageDir, join(entry.parentPath, entry.name)),
          ),
        )
    : [];
  const unexpected = present.filter((path) => !expected.includes(path)).sort();
  // A step's output is as old as its oldest file. The build writes
  // every output after reading the step's inputs, so an input newer
  // than that was edited, checked out or pulled after the build. Steps
  // with missing output are already reported as `missing`.
  const stale = buildSteps(dirs)
    .filter(({ outputs }) => outputs.every((path) => existsSync(path)))
    .flatMap(({ inputs, outputs }) => {
      const builtAt = Math.min(
        ...outputs.map((path) => statSync(path).mtimeMs),
      );
      return inputs.filter(
        (path) => existsSync(path) && statSync(path).mtimeMs > builtAt,
      );
    })
    .map((path) => toPosix(relative(dirs.repoDir, path)))
    .sort();
  return { missing, unexpected, stale };
}
