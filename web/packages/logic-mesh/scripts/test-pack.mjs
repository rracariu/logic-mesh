// `npm run test:pack`: packs the built package, checks the tarball
// holds exactly the expected files, installs it with zod (its peer)
// into a fresh project outside the repository, and runs
// pack-smoke.mjs there with plain `node`. This is the path a consumer
// takes and the vitest suite does not: the suite imports src/ through
// an alias, never the published `exports`.
//
// The scratch project lives under the OS temp dir (`TMPDIR`); it is
// removed on success and kept for inspection on failure.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  checkBuildOutput,
  expectedPackageFiles,
  packageDir,
  repoDir,
} from './package-files.mjs';

const manifest = JSON.parse(
  readFileSync(join(packageDir, 'package.json'), 'utf8'),
);

// logic_mesh.js instantiates the wasm on import and exports nothing of
// its own: a bundler that took it for side-effect free would drop the
// instantiation and leave every engine call failing.
assert.ok(
  manifest.sideEffects.includes('./dist/logic_mesh.js'),
  'sideEffects keeps the wasm-pack glue',
);

// The smoke test runs against the oldest zod the peer range admits,
// the version most likely to break; the vitest suite covers the newest.
const zodFloor = /^\^(\d+\.\d+\.\d+)$/.exec(manifest.peerDependencies.zod)?.[1];
assert.ok(zodFloor, 'peerDependencies.zod is a caret range');

// Under `npm run`, reuse the npm that started this script.
const npmCli = process.env.npm_execpath;

/** Runs npm in `cwd` and returns its stdout. */
function npm(args, cwd) {
  const options = {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'inherit'],
  };
  return npmCli
    ? execFileSync(process.execPath, [npmCli, ...args], options)
    : execFileSync('npm', args, {
        ...options,
        shell: process.platform === 'win32',
      });
}

const { missing, unexpected, stale } = checkBuildOutput();
if (missing.length > 0 || unexpected.length > 0 || stale.length > 0) {
  console.error({ missing, unexpected, stale });
  console.error('test:pack: run `npm run build` (or `build:dev`) first.');
  process.exit(1);
}

const workDir = mkdtempSync(join(tmpdir(), 'logic-mesh-pack-'));
let passed = false;
try {
  const [packed] = JSON.parse(
    npm(
      ['pack', '--json', '--loglevel=warn', '--pack-destination', workDir],
      packageDir,
    ),
  );
  assert.deepEqual(
    packed.files.map((file) => file.path).sort(),
    expectedPackageFiles(),
    'tarball contents',
  );

  const appDir = join(workDir, 'app');
  mkdirSync(appDir);
  writeFileSync(
    join(appDir, 'package.json'),
    JSON.stringify({ name: 'logic-mesh-pack-smoke', private: true }),
  );
  npm(
    [
      'install',
      '--no-audit',
      '--no-fund',
      '--loglevel=warn',
      join(workDir, packed.filename),
      `zod@${zodFloor}`,
    ],
    appDir,
  );

  assert.equal(
    readFileSync(join(appDir, 'node_modules', 'logic-mesh', 'LICENSE'), 'utf8'),
    readFileSync(join(repoDir, 'LICENSE'), 'utf8'),
    'the shipped LICENSE is the repository root LICENSE',
  );

  copyFileSync(
    join(packageDir, 'scripts', 'pack-smoke.mjs'),
    join(appDir, 'smoke.mjs'),
  );
  execFileSync(process.execPath, ['smoke.mjs'], {
    cwd: appDir,
    stdio: 'inherit',
  });
  passed = true;
  console.log(`test:pack: ${packed.filename} passed`);
} finally {
  if (passed) {
    rmSync(workDir, { recursive: true, force: true });
  } else {
    console.error(`test:pack: scratch project kept at ${workDir}`);
  }
}
