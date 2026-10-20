// Type-level tests of the wasm bindings' declarations, checked by `tsc`
// through vitest's typecheck mode (see vitest.config.ts) and never run.
import { describe, expectTypeOf, it } from 'vitest';
import type {
  BlockDesc,
  BlockInspection,
  BlockNotification,
  BlocksEngine,
  EngineCommand,
  JsBlockFactory,
  JsConnector,
  LinkData,
  LogLevel,
  Program,
} from '../src/index';

// Regression: the generated declarations took and returned `any` for
// every structured value.
describe('the binding declarations', () => {
  it('name the package types', () => {
    expectTypeOf<EngineCommand['loadProgram']>()
      .parameter(0)
      .toEqualTypeOf<Program>();
    expectTypeOf<ReturnType<EngineCommand['getProgram']>>().toEqualTypeOf<
      Promise<Program>
    >();
    expectTypeOf<ReturnType<EngineCommand['createLink']>>().toEqualTypeOf<
      Promise<LinkData>
    >();
    expectTypeOf<ReturnType<EngineCommand['inspectBlock']>>().toEqualTypeOf<
      Promise<BlockInspection>
    >();
    expectTypeOf<ReturnType<EngineCommand['evalBlock']>>().toEqualTypeOf<
      Promise<unknown[]>
    >();
    expectTypeOf<ReturnType<EngineCommand['writeBlockInput']>>().toEqualTypeOf<
      Promise<unknown>
    >();
    expectTypeOf<EngineCommand['createWatch']>()
      .parameter(0)
      .toEqualTypeOf<(notification: BlockNotification) => void>();
    expectTypeOf<BlocksEngine['registerBlock']>()
      .parameter(0)
      .toEqualTypeOf<BlockDesc>();
    expectTypeOf<BlocksEngine['registerConnector']>()
      .parameter(1)
      .toEqualTypeOf<JsConnector>();
    expectTypeOf<ReturnType<BlocksEngine['listBlocks']>>().toEqualTypeOf<
      BlockDesc[]
    >();
  });

  it('let the block factory be left out', () => {
    const register = (engine: BlocksEngine, desc: BlockDesc) => {
      engine.registerBlock(desc);
      engine.registerBlock(desc, null);
      const factory: JsBlockFactory = () => (inputs) => [inputs[0]];
      engine.registerBlock(desc, factory);
      // A synchronous executor is a valid one.
      engine.registerBlock(desc, () => () => undefined);
    };
    expectTypeOf(register).toBeFunction();
  });

  it('take the scan interval as a number', () => {
    expectTypeOf<ConstructorParameters<typeof BlocksEngine>[0]>().toEqualTypeOf<
      number | null | undefined
    >();
  });

  it('type the log levels', () => {
    expectTypeOf<LogLevel>().toEqualTypeOf<
      'off' | 'error' | 'warn' | 'info' | 'debug' | 'trace'
    >();
  });

  // Regression: an `interface` LinkData could not be stored as a graph
  // library's `Record<string, unknown>` edge data.
  it('let a link be stored as JSON object data', () => {
    expectTypeOf<LinkData>().toExtend<Record<string, unknown>>();
  });
});
