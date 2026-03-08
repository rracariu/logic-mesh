import type { BlockDesc, Kind } from "logic-mesh/index.d.ts";
import type { BlocksEngine } from "logic-mesh/logic_mesh.d.ts";
import {
  z,
  ZodArray,
  ZodBoolean,
  ZodDefault,
  ZodEnum,
  ZodNumber,
  ZodObject,
  ZodOptional,
  ZodString,
  type ZodType,
  ZodUnknown,
} from "zod";

type InferType<T> = {
  -readonly [K in keyof T]: z.infer<ExtractZodType<T[K]>>;
};
type ExtractZodType<T> = T extends readonly [string, infer U extends ZodType]
  ? U
  : never;

type BlockDetails = Omit<BlockDesc, "inputs" | "outputs" | "implementation">;
type TupleType = readonly [
  readonly [string, ZodType],
  ...(readonly [string, ZodType])[],
];

/**
 * Defines a new block type with the specified configuration.
 * @param config - The block configuration, including description, input/output types, and execution logic.
 * @returns A class that extends TypedBlock, which can be registered with the BlocksEngine.
 */
export function defineBlock<I extends TupleType, O extends TupleType>(config: {
  desc: BlockDetails;
  inputs: I;
  outputs: O;
  execute?: (inputs: InferType<I>) => Promise<unknown[] | undefined>;
}) {
  const { execute: executeFn, ...blockConfig } = config;

  return class extends TypedBlock<I, O> {
    constructor() {
      super(blockConfig);
    }

    override execute(inputs: InferType<I>): Promise<unknown[] | undefined> {
      if (executeFn) return executeFn(inputs);
      throw new Error("Not implemented");
    }

    static register(engine: BlocksEngine) {
      const executor = new (this as any)();
      engine.registerBlock(executor.desc, () =>
        executor.executeImpl.bind(executor),
      );
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

    const blockDescription = desc as BlockDesc;
    blockDescription.implementation = "external";

    blockDescription.inputs = this.inputs.map((name, i) => ({
      name,
      kind: this.zodToKind(this.inputTypes.at(i) as ZodType),
    }));

    blockDescription.outputs = this.outputs.map((name, i) => ({
      name,
      kind: this.zodToKind(this.outputTypes.at(i) as ZodType),
    }));

    this.desc = blockDescription;
  }

  execute([..._inputs]: InferType<I>): Promise<unknown[] | undefined> {
    throw new Error("Not implemented");
  }

  static register(_engine: BlocksEngine) {
    throw new Error("Not implemented");
  }

  protected async executeImpl(inputs: InferType<I>) {
    // Validate inputs
    if (inputs.length !== this.inputs.length) {
      throw new Error("Invalid number of inputs");
    }

    this.inputs.map((_, i) => {
      const val = inputs.at(i);
      const kind = this.inputTypes[i] as ZodType;
      inputs[i] = kind.parse(val);
    });

    const res = await this.execute(inputs);
    if (res === undefined) {
      return undefined;
    }

    // Validate outputs
    if (res.length !== this.outputs.length) {
      throw new Error("Invalid number of outputs");
    }

    this.outputs.forEach((_, i) => {
      const val = res.at(i);
      const kind = this.outputTypes[i] as ZodType;
      kind.parse(val);
    });

    return res;
  }

  private zodToKind(kind: ZodType | undefined): Kind {
    if (kind === undefined) {
      throw new Error("Unspecified kind");
    }

    if (kind instanceof ZodOptional) {
      kind = kind.def.innerType as ZodType;
    }

    if (kind instanceof ZodBoolean) {
      return "bool";
    } else if (kind instanceof ZodNumber) {
      return "number";
    } else if (kind instanceof ZodString) {
      return "str";
    } else if (kind instanceof ZodEnum) {
      return "str";
    } else if (kind instanceof ZodArray) {
      return "list";
    } else if (kind instanceof ZodObject) {
      return "dict";
    } else if (kind instanceof ZodUnknown) {
      return "null";
    } else if (kind instanceof ZodDefault) {
      return this.zodToKind(kind.def.innerType as ZodType);
    } else {
      throw new Error(`Invalid kind: ${kind?.constructor.name}`);
    }
  }
}
