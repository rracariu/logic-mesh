export {
  BlocksEngine,
  initEngine,
  EngineCommand,
  registerConnector,
  unregisterConnector,
  connectorRegistered,
  connectorIs,
} from './logic_mesh.js';
export { defineBlock, TypedBlock } from './TypedBlock.js';
export {
  createEngineSession,
  startEngine,
  EngineSession,
  type EngineSessionOptions,
} from './EngineSession.js';
export {
  defineJsBlocks,
  type JsBlocks,
  type JsBlockFn,
  type JsBlocksOptions,
} from './JsBlocks.js';

/**
 * The callback a connector subscription pushes values through.
 *
 * - `callback(value)` pushes `value` into the subscription stream.
 * - `callback(undefined, detail)` pushes an error carrying `detail`;
 *   the subscription stays live.
 * - `callback()` — no arguments — ends the stream.
 *
 * Beware: an accidentally-`undefined` value is indistinguishable from
 * `callback()` and silently ends the subscription. Once the engine
 * drops the subscription the callback is destroyed and calling it
 * throws, so stop calling it after the unsubscribe function runs.
 */
export type ConnectorCallback = (value?: unknown, detail?: unknown) => void;

/**
 * A connector that is implemented in JS.
 *
 * Register it under a name with `registerConnector` (or the
 * `BlocksEngine.registerConnector` convenience method), then attach it
 * to the running engine with the `addConnector` engine command; the
 * `ExternalIn`/`ExternalOut`/`Request` blocks resolve it by name.
 *
 * Every method may return a Promise — the engine awaits it — or a
 * plain value, and is invoked with `this` bound to the connector
 * object, so class instances work. Values cross the boundary as
 * Haystack-encoded JSON: plain JS numbers, strings, booleans and
 * `null` map directly.
 */
export interface JsConnector {
  /**
   * Called once per subscription. Push values through `callback` and
   * return an unsubscribe function — directly or via a Promise — to be
   * told when the engine drops the subscription (return nothing if
   * there is no cleanup to do). The unsubscribe function may run after
   * the stream already ended, so it must be idempotent.
   */
  subscribe(
    address: string,
    callback: ConnectorCallback,
  ): (() => void) | void | Promise<(() => void) | void>;

  /**
   * Publishes `value` to `address`. A thrown exception or a rejected
   * Promise marks the publish as failed.
   */
  publish(address: string, value: unknown): void | Promise<void>;

  /**
   * Round-trips `value` through `address`, resolving with the
   * response value.
   */
  request(address: string, value: unknown): unknown;

  /**
   * Optional. Called when the engine starts running, before any block
   * executes.
   */
  start?(): void | Promise<void>;

  /**
   * Optional. Called on engine shutdown and reset. Must end every
   * outstanding subscription by invoking its callback with no
   * arguments.
   */
  stop?(): void | Promise<void>;
}

/**
 * The kind of the block pin.
 */
export type Kind =
  | 'null'
  | 'remove'
  | 'marker'
  | 'na'
  | 'bool'
  | 'number'
  | 'str'
  | 'uri'
  | 'ref'
  | 'symbol'
  | 'date'
  | 'time'
  | 'dateTime'
  | 'coord'
  | 'xstr'
  | 'list'
  | 'dict'
  | 'grid';

/**
 * A block that is implemented in JS
 */
export type JsBlock = {
  /**
   * The block description
   */
  desc: BlockDesc;

  /**
   * An optional block factory function that returns a function that is called when the block is executed.
   *
   * The execute function is called with the block inputs and should return the block outputs.
   * The order of the inputs and outputs is the same as the order of the pins in the block description.
   * Returning `undefined` will not change the output value.
   *
   * @returns The execute function that is called when the block is executed.
   */
  executor?: () => (inputs: unknown[]) => Promise<unknown[] | undefined>;
};

/**
 * A block pin.
 *
 * Pins are the inputs and outputs of a block.
 */
export interface BlockPin {
  /**
   * The pin name
   */
  name: string;

  /**
   * The pin kind
   * Must be a valid haystack type kind.
   * See https://project-haystack.org/doc/docHaystack/Kinds
   */
  kind: Kind;

  /**
   * The pin value
   * Value is a Haystack value encoded as JSON.
   */
  value?: unknown;

  /**
   * True if the pin is connected to another pin.
   */
  isConnected?: boolean;
}

/**
 * Describe a block that is available in block library.
 */
export interface BlockDesc {
  /**
   * The block name
   */
  name: string;

  /**
   * The block display name
   */
  dis: string;

  /**
   * The block library name
   */
  lib: string;

  /**
   * The block library version
   */
  ver: string;

  /**
   * The block category
   */
  category: string;

  /**
   * The block documentation
   */
  doc: string;

  /**
   * The block implementation
   */
  implementation: 'native' | 'external';

  /**
   * The block inputs
   */
  inputs: BlockPin[];

  /**
   * The block outputs
   */
  outputs: BlockPin[];

  /**
   * The block run condition.
   *
   * If not set, the block will be executed when any of its inputs change.
   * Otherwise, the block will execute regularly according to the run condition.
   *
   * Default: 'change'
   */
  runCondition?: 'change' | 'always';
}

/**
 * Notification on a block change
 */
export interface BlockNotification {
  /**
   * The block id
   */
  id: string;

  /**
   * The changes
   */
  changes: {
    /**
     * The block pin name
     */
    name: string;
    /**
     * The block pin source
     */
    source: string;
    /**
     * The value that was changed on this pin. The shape is
     * Haystack-encoded JSON; consumers narrow as needed (e.g.
     * `typeof value === 'number'` for a numeric pin).
     */
    value: unknown;
  }[];

  /**
   * The block's operational state at the time of the notification.
   * Drives fault visualization on the UI.
   */
  state: 'running' | 'fault' | 'disabled' | 'terminated';

  /**
   * Optional reason associated with `state === "fault"`.
   */
  faultReason?: string;
}

export interface LinkData {
  /**
   * The link id
   */
  id?: string;

  /**
   * The link source block pin name
   */
  sourceBlockPinName: string;

  /**
   * The link source block uuid
   */
  sourceBlockUuid: string;

  /**
   * The link target block pin name
   */
  targetBlockPinName: string;

  /**
   * The link target block uuid
   */
  targetBlockUuid: string;
}

/**
 * Describes a program that would be loaded in the engine.
 *
 * The program is a set of blocks and links between them.
 * The program and the blocks have meta data that would be used in the editor
 * for example to position the blocks.
 */
export interface Program {
  /**
   * The program name
   */
  name: string;

  /**
   * Optional program description
   */
  description?: string;

  /**
   * Blocks used in the program
   */
  blocks: {
    [blockUuid: string]: {
      name: string;
      lib: string;

      /** User-supplied display label shown alongside the block-type name. */
      label?: string;

      /**
       * UI widget identity for ExternalIn/ExternalOut blocks placed as
       * widgets. Editor-only metadata; the engine ignores it.
       */
      widget?: {
        kind: string;
        config?: Record<string, unknown>;
        configSources?: Record<string, string>;
        valueSource?: string;
      };

      positions: {
        x: number;
        y: number;
      };

      inputs?: {
        [pinName: string]: { value?: unknown; isConnected?: boolean };
      };

      outputs?: {
        [pinName: string]: { value?: unknown; isConnected?: boolean };
      };
    };
  };

  /**
   * Links between blocks
   */
  links: {
    [linkUuid: string]: LinkData;
  };
}
