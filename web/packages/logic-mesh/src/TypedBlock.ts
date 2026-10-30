import type { BlockDesc, Kind } from './index.js';
import type { BlocksEngine } from './logic_mesh.js';
import {
  z,
  ZodArray,
  ZodBoolean,
  ZodDefault,
  ZodEnum,
  ZodLiteral,
  ZodNull,
  ZodNullable,
  ZodNumber,
  ZodObject,
  ZodOptional,
  ZodString,
  type ZodType,
  ZodUnion,
  ZodUnknown,
} from 'zod';

type InferType<T> = {
  -readonly [K in keyof T]: z.infer<ExtractZodType<T[K]>>;
};
type ExtractZodType<T> = T extends readonly [string, infer U extends ZodType]
  ? U
  : never;

type BlockDetails = Omit<BlockDesc, 'inputs' | 'outputs' | 'implementation'>;
type TupleType = readonly (readonly [string, ZodType])[];

/**
 * Defines a new block type with the specified configuration.
 *
 * Execution semantics: the engine passes `null` for an input pin
 * whose value has not been set yet — e.g. on the first cycles after a
 * link is created, before any value has flowed — or is `Null`, and the
 * block reads it as `undefined`. While any input whose schema rejects
 * `undefined` (a plain `z.number()`, say) is still unset, execution is
 * skipped — a no-op, not a fault — and `execute` is not called. Wrap a
 * pin's schema in `.optional()` or `.default(...)` to let the executor
 * run without that pin (a default materializes as the argument value).
 * A pin with a present but mistyped value still faults the block: only
 * absence is tolerated.
 *
 * @param config - The block configuration, including description, input/output types, and execution logic.
 * @returns A class that extends TypedBlock, which can be registered with the BlocksEngine.
 */
export function defineBlock<I extends TupleType, O extends TupleType>(config: {
  desc: BlockDetails;
  inputs: I;
  outputs: O;
  execute?: (inputs: InferType<I>) => Promise<InferType<O> | undefined>;
}) {
  const { execute: executeFn, ...blockConfig } = config;

  return class extends TypedBlock<I, O> {
    constructor() {
      super(blockConfig);
    }

    override execute(inputs: InferType<I>): Promise<InferType<O> | undefined> {
      if (executeFn) return executeFn(inputs);
      throw new Error('Not implemented');
    }

    static register(engine: BlocksEngine) {
      // `this` here is the anonymous class produced by `defineBlock`.
      // Cast to its construct signature so we can `new` it without
      // falling back to `any` — the runtime shape matches the
      // `TypedBlock<I, O>` instance type by construction (we just
      // declared the class as `class extends TypedBlock<I, O>`).
      const Ctor = this as unknown as new () => TypedBlock<I, O>;
      const executor = new Ctor();
      if (executeFn) {
        // The engine hands the inputs over as they are; `executeImpl`
        // validates them against the input schemas.
        engine.registerBlock(
          executor.desc,
          () => (inputs: unknown[]) =>
            executor.executeImpl(inputs as InferType<I>),
        );
      } else {
        engine.registerBlock(executor.desc);
      }
    }
  };
}

export class TypedBlock<I extends TupleType, O extends TupleType> {
  readonly desc: BlockDesc;

  readonly inputs: string[];
  readonly inputTypes: InferType<I>;

  readonly outputs: string[];
  readonly outputTypes: InferType<O>;

  constructor({
    desc,
    inputs,
    outputs,
  }: {
    desc: BlockDetails;
    inputs: I;
    outputs: O;
  }) {
    this.inputs = inputs.map(([name, _]) => name);
    this.inputTypes = inputs.map(([, type]) => type) as InferType<I>;

    this.outputs = outputs.map(([name, _]) => name);
    this.outputTypes = outputs.map(([, type]) => type) as InferType<O>;

    // A copy: the caller's `desc` may describe more than one block.
    this.desc = {
      ...desc,
      implementation: 'external',
      inputs: this.inputs.map((name, i) => ({
        name,
        kind: this.zodToKind(this.inputTypes.at(i) as ZodType),
      })),
      outputs: this.outputs.map((name, i) => ({
        name,
        kind: this.zodToKind(this.outputTypes.at(i) as ZodType),
      })),
    };
  }

  execute([..._inputs]: InferType<I>): Promise<unknown[] | undefined> {
    throw new Error('Not implemented');
  }

  static register(_engine: BlocksEngine) {
    throw new Error('Not implemented');
  }

  async executeImpl(inputs: InferType<I>) {
    // Validate inputs
    if (inputs.length !== this.inputs.length) {
      throw new Error('Invalid number of inputs');
    }

    // The engine passes `null` for a pin whose value is not yet set —
    // e.g. on the first cycles after a link is created, before a value
    // flows — or is Null, and a direct caller may pass `undefined`.
    // That must not fault the block permanently: an unset pin, either
    // way, is probed with `safeParse`, and when its schema rejects
    // `undefined` (a required pin like plain `z.number()`) the whole
    // execution is skipped by returning a non-array — on the Rust side
    // that is a silent no-op: outputs untouched, no fault. A schema
    // that accepts `undefined` (`.optional()`, `.default(...)`,
    // `z.unknown()`) contributes its parsed value instead — a default
    // materializes here. A pin with a DEFINED value keeps the strict
    // `parse`: a present-but-mistyped value is a real error and still
    // faults, deliberately.
    for (let i = 0; i < this.inputs.length; i++) {
      const val = inputs.at(i);
      const kind = this.inputTypes[i] as ZodType;
      if (val === undefined || val === null) {
        const probe = kind.safeParse(undefined);
        if (!probe.success) return undefined; // not ready yet — skip
        inputs[i] = probe.data;
      } else {
        inputs[i] = kind.parse(val);
      }
    }

    const res = await this.execute(inputs);
    if (res === undefined) {
      return undefined;
    }

    // Validate outputs
    if (res.length !== this.outputs.length) {
      throw new Error('Invalid number of outputs');
    }

    this.outputs.forEach((_, i) => {
      const val = res.at(i);
      const kind = this.outputTypes[i] as ZodType;
      kind.parse(val);
    });

    return res;
  }

  /**
   * The pin kind of a Zod schema.
   *
   * `.optional()`, `.nullable()` and `.default(...)` take the kind of
   * the schema they wrap. A literal, and a union whose members all
   * have one kind (`z.null()` members aside, as with `.nullable()`),
   * take that kind — so a union of string literals is a `'str'` pin.
   * Any other schema throws.
   */
  zodToKind(kind: ZodType | undefined): Kind {
    if (kind === undefined) {
      throw new Error('Unspecified kind');
    }

    if (
      kind instanceof ZodOptional ||
      kind instanceof ZodNullable ||
      kind instanceof ZodDefault
    ) {
      return this.zodToKind(kind.def.innerType as ZodType);
    }

    if (kind instanceof ZodLiteral) {
      return sameKind(
        [...kind.values].map((value) => literalKind(value)),
        'literal',
      );
    }

    if (kind instanceof ZodUnion) {
      const members = (kind.options as ZodType[]).filter(
        (member) => !(member instanceof ZodNull),
      );
      return sameKind(
        members.map((member) => this.zodToKind(member)),
        'union',
      );
    }

    if (kind instanceof ZodBoolean) {
      return 'bool';
    } else if (kind instanceof ZodNumber) {
      return 'number';
    } else if (kind instanceof ZodString) {
      return 'str';
    } else if (kind instanceof ZodEnum) {
      return 'str';
    } else if (kind instanceof ZodArray) {
      return 'list';
    } else if (kind instanceof ZodObject) {
      return 'dict';
    } else if (kind instanceof ZodUnknown) {
      return 'null';
    } else {
      throw new Error(
        `Invalid kind: ${kind.constructor.name} has no pin kind. Use a ` +
          'boolean, number, string, enum, literal, array, object or ' +
          'unknown schema, a union of schemas of one kind, or one of ' +
          'these made optional, nullable or defaulted.',
      );
    }
  }
}

/** The pin kind of a literal value. */
function literalKind(value: unknown): Kind {
  switch (typeof value) {
    case 'string':
      return 'str';
    case 'number':
      return 'number';
    case 'boolean':
      return 'bool';
    default:
      throw new Error(
        `Invalid kind: the literal ${String(value)} has no pin kind. ` +
          'Use string, number or boolean literals.',
      );
  }
}

/** The one kind of `kinds`, which describe the members of a `what`. */
function sameKind(kinds: Kind[], what: string): Kind {
  const unique = [...new Set(kinds)];
  if (unique.length !== 1) {
    throw new Error(
      `Invalid kind: a ${what} with ${unique.length === 0 ? 'no kind' : `the kinds ${unique.join(', ')}`} has no ` +
        'single pin kind. Use members of one kind.',
    );
  }
  return unique[0] as Kind;
}
