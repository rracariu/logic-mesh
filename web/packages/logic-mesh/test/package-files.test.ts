// Tests for the build-output checks shared by the `prepublishOnly`
// guard and `test:pack`. Each test lays out a miniature repository
// (the crate at the root, the package under web/packages/logic-mesh)
// in a temp dir and dates its files explicitly, so the outcome never
// depends on how fast the files were written.
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  checkBuildOutput,
  expectedPackageFiles,
} from '../scripts/package-files.mjs';

/** When the fixture's sources were last written (Unix seconds). */
const SOURCES_AT = 1_700_000_000;
/** When the fixture's build ran, after its sources. */
const BUILT_AT = SOURCES_AT + 60;
/** When a file is touched after the build. */
const EDITED_AT = BUILT_AT + 60;

let repoDir: string;
let packageDir: string;

/** Writes `path` (creating its directory) and sets its times to `at`. */
function writeAt(path: string, at: number) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, '');
  utimesSync(path, at, at);
}

beforeEach(() => {
  repoDir = mkdtempSync(join(tmpdir(), 'logic-mesh-files-'));
  packageDir = join(repoDir, 'web', 'packages', 'logic-mesh');
  for (const path of [
    'LICENSE',
    'README.md',
    'Cargo.toml',
    'Cargo.lock',
    'build.rs',
    'src/lib.rs',
    'src/blocks/math/add.rs',
    'block_macro/Cargo.toml',
    'block_macro/src/lib.rs',
  ]) {
    writeAt(join(repoDir, path), SOURCES_AT);
  }
  for (const path of ['tsconfig.json', 'src/index.ts', 'src/Mod.ts']) {
    writeAt(join(packageDir, path), SOURCES_AT);
  }
  // A complete build: every file the tarball must hold.
  for (const path of expectedPackageFiles({ packageDir, repoDir })) {
    writeAt(join(packageDir, path), BUILT_AT);
  }
});

afterEach(() => {
  rmSync(repoDir, { recursive: true, force: true });
});

describe('checkBuildOutput', () => {
  it('accepts a complete build made after its sources', () => {
    expect(checkBuildOutput({ packageDir, repoDir })).toEqual({
      missing: [],
      unexpected: [],
      stale: [],
    });
  });

  it('reports missing build output and leftover files', () => {
    rmSync(join(packageDir, 'dist', 'Mod.js'));
    writeAt(join(packageDir, 'dist', 'Old.js'), BUILT_AT);

    expect(checkBuildOutput({ packageDir, repoDir })).toEqual({
      missing: ['dist/Mod.js'],
      unexpected: ['dist/Old.js'],
      stale: [],
    });
  });

  it('reports a TypeScript source edited after the build', () => {
    writeAt(join(packageDir, 'src', 'Mod.ts'), EDITED_AT);
    writeAt(join(packageDir, 'tsconfig.json'), EDITED_AT);

    expect(checkBuildOutput({ packageDir, repoDir }).stale).toEqual([
      'web/packages/logic-mesh/src/Mod.ts',
      'web/packages/logic-mesh/tsconfig.json',
    ]);
  });

  it('reports crate sources and manifests changed after the build', () => {
    writeAt(join(repoDir, 'Cargo.toml'), EDITED_AT);
    writeAt(join(repoDir, 'src', 'blocks', 'math', 'add.rs'), EDITED_AT);
    writeAt(join(repoDir, 'block_macro', 'src', 'lib.rs'), EDITED_AT);

    expect(checkBuildOutput({ packageDir, repoDir }).stale).toEqual([
      'Cargo.toml',
      'block_macro/src/lib.rs',
      'src/blocks/math/add.rs',
    ]);
  });

  it('reports a root LICENSE changed after the build copied it', () => {
    writeAt(join(repoDir, 'LICENSE'), EDITED_AT);

    expect(checkBuildOutput({ packageDir, repoDir }).stale).toEqual([
      'LICENSE',
    ]);
  });

  it('ignores files the build does not read', () => {
    writeAt(join(repoDir, 'README.md'), EDITED_AT);
    writeAt(join(repoDir, 'src', '.DS_Store'), EDITED_AT);
    writeAt(join(packageDir, 'README.md'), EDITED_AT);
    writeAt(join(packageDir, 'package.json'), EDITED_AT);

    expect(checkBuildOutput({ packageDir, repoDir }).stale).toEqual([]);
  });
});
