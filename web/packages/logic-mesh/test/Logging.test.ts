// The module start hook and the console log level, against the real
// wasm engine in Node.
import { describe, expect, it } from 'vitest';
// The generated module itself, which the package does not export: its
// start hook runs once when the module loads.
import { start } from '../dist/logic_mesh.js';
import * as logicMesh from '../src/index';
import { isLogicMeshError, setLogLevel, startEngine } from '../src/index';

describe('the module start hook', () => {
  // Regression: a second run failed to install the console logger
  // again, and the panic that followed aborted the wasm instance.
  it('runs again without panicking', async () => {
    start();
    start();

    const session = startEngine({ scanInterval: 10 });
    try {
      expect(await session.command.evalBlock('Add', [1, 2])).toStrictEqual([3]);
    } finally {
      await session.stop();
    }
  });

  it('is not exported by the package, and neither is the libhaystack panic hook', () => {
    const exported = Object.keys(logicMesh);
    expect(exported).not.toContain('start');
    expect(exported).not.toContain('init_panic_hook');
  });
});

describe('setLogLevel', () => {
  it('takes the log levels', () => {
    for (const level of [
      'off',
      'error',
      'info',
      'debug',
      'trace',
      'warn',
    ] as const) {
      expect(() => setLogLevel(level)).not.toThrow();
    }
  });

  it('throws a LogicMeshError for any other level', () => {
    let thrown: unknown;
    try {
      setLogLevel('loud' as 'warn');
    } catch (err) {
      thrown = err;
    }
    expect(isLogicMeshError(thrown) && thrown.code).toBe('InvalidArgument');
  });
});
