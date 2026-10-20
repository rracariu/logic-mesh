// The module start hook and the console log level, against the real
// wasm engine in Node.
import { describe, expect, it, vi } from 'vitest';
// The generated module itself, which the package does not export: its
// start hook runs once when the module loads.
import { start } from '../dist/logic_mesh.js';
import type { BlockDesc, EngineCommand } from '../src/index';
import * as logicMesh from '../src/index';
import {
  createEngineSession,
  isLogicMeshError,
  setLogLevel,
  startEngine,
} from '../src/index';

/** Polls block `id` until it is in Fault, or fails after `timeoutMs`.
 * Asks the engine rather than waiting on a notification: a fault set
 * while a block waits on its inputs is only reported once the wait
 * ends. Each inspection cuts the block's cycle short, so the polls are
 * further apart than the engines' scan interval. */
async function untilFaulted(
  command: EngineCommand,
  id: string,
  what: string,
  timeoutMs = 3000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while ((await command.inspectBlock(id)).state !== 'fault') {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

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

describe('block faults', () => {
  // Regression: a JS block's fault, Sub's and a failed input conversion
  // were logged at error, on every cycle that faulted, flooding the
  // console at the default level — though every fault reaches JS
  // through the block's state and fault reason.
  it('are not logged at the default level', async () => {
    setLogLevel('warn');
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    const warnings = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const session = createEngineSession({ scanInterval: 20 });
    try {
      const thrower: BlockDesc = {
        name: 'Thrower',
        dis: 'Thrower',
        lib: `logging-${crypto.randomUUID()}`,
        ver: '0.0.1',
        category: 'Test',
        doc: 'Throws on every cycle.',
        implementation: 'external',
        inputs: [{ name: 'in', kind: 'number' }],
        outputs: [{ name: 'out', kind: 'number' }],
        runCondition: 'always',
      };
      session.engine.registerBlock(thrower, () => () => {
        throw new Error('boom');
      });
      session.start();
      const { command } = session;

      const throws = await command.addBlock(
        thrower.name,
        undefined,
        thrower.lib,
      );
      const sub = await command.addBlock('Sub');
      await command.writeBlockInput(sub, 'a', {
        _kind: 'number',
        val: 1,
        unit: 'm',
      });
      await command.writeBlockInput(sub, 'b', {
        _kind: 'number',
        val: 1,
        unit: 's',
      });
      const source = await command.addBlock('Add');
      const converts = await command.addBlock('Add');
      await command.createLink(source, converts, 'out', 'in0');
      await command.writeBlockOutput(source, 'out', 'not a number');

      const faulting = { throws, sub, converts };
      for (const [name, id] of Object.entries(faulting)) {
        await untilFaulted(command, id, `the ${name} block to fault`);
      }
      // The thrower faults again on every cycle meanwhile.
      await new Promise((resolve) => setTimeout(resolve, 200));
      expect(errors).not.toHaveBeenCalled();
      expect(warnings).not.toHaveBeenCalled();
    } finally {
      errors.mockRestore();
      warnings.mockRestore();
      await session.stop();
    }
  });
});
