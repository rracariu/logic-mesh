// Consumer-side smoke test, run by test-pack.mjs with plain `node`
// inside a throwaway project that installed the packed tarball and
// zod: it resolves `logic-mesh` exactly as a user's code would, from
// node_modules, through the package's `exports`.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { defineBlock, initEngine, startEngine } from 'logic-mesh';
import manifest from 'logic-mesh/package.json' with { type: 'json' };
import { z } from 'zod';

const require = createRequire(import.meta.url);
const entry = fileURLToPath(import.meta.resolve('logic-mesh'));

assert.equal(manifest.name, 'logic-mesh', 'the ./package.json export');
// Tools resolving with `require` conditions reach the entry through the
// `default` condition. Only resolved, not loaded: `require()` of the
// wasm module needs Node.js 26.
assert.equal(require.resolve('logic-mesh'), entry, 'the default condition');
// The package must see the consumer's zod (the peer dependency), not a
// private copy installed beside it.
assert.equal(
  createRequire(entry).resolve('zod'),
  require.resolve('zod'),
  'logic-mesh resolves the consumer zod',
);

// Loading the module instantiated the wasm and registered the core
// library.
const blocks = initEngine().listBlocks();
assert.ok(
  blocks.some((block) => block.name === 'Add' && block.lib === 'core'),
  'the core Add block is registered',
);

// A running engine evaluates a core block.
const session = startEngine();
try {
  assert.deepEqual(await session.command.evalBlock('Add', [1, 2]), [3]);
} finally {
  await session.stop();
}

// `defineBlock` maps the consumer's own zod schemas to pin kinds: the
// peer dependency must resolve to the same zod instance.
const Scale = defineBlock({
  desc: {
    name: 'Scale',
    dis: 'Scale',
    lib: 'smoke',
    ver: '0.0.1',
    category: 'Smoke',
    doc: 'Doubles its input',
  },
  inputs: [['in', z.number()]],
  outputs: [['out', z.number()]],
});
assert.deepEqual(
  new Scale().desc.inputs.map((pin) => pin.kind),
  ['number'],
  'zod schemas from the consumer map to pin kinds',
);

console.log(
  `logic-mesh ${manifest.version}: ${blocks.length} blocks, Add(1, 2) = 3`,
);
