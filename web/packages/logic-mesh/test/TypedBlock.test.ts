import { describe, expect, it, vi } from "vitest";
import { z } from "zod";
import { defineBlock } from "../src/TypedBlock";

const desc = {
  name: "TestBlock",
  dis: "Test Block",
  lib: "test",
  ver: "0.0.1",
  category: "Test",
  doc: "A test block",
};

describe("defineBlock", () => {
  it("populates desc, inputs, and outputs", () => {
    const Block = defineBlock({
      desc,
      inputs: [["x", z.number()]] as const,
      outputs: [["y", z.string()]] as const,
    });
    const b = new Block();

    expect(b.desc.name).toBe("TestBlock");
    expect(b.desc.implementation).toBe("external");
    expect(b.inputs).toEqual(["x"]);
    expect(b.outputs).toEqual(["y"]);
    expect(b.desc.inputs).toEqual([{ name: "x", kind: "number" }]);
    expect(b.desc.outputs).toEqual([{ name: "y", kind: "str" }]);
  });

  it("supports multiple inputs and outputs", () => {
    const Block = defineBlock({
      desc,
      inputs: [
        ["a", z.string()],
        ["b", z.boolean()],
      ] as const,
      outputs: [["result", z.number()]] as const,
    });
    const b = new Block();

    expect(b.inputs).toEqual(["a", "b"]);
    expect(b.desc.inputs).toEqual([
      { name: "a", kind: "str" },
      { name: "b", kind: "bool" },
    ]);
  });

  it("registers with the engine", () => {
    const Block = defineBlock({
      desc,
      inputs: [["x", z.number()]] as const,
      outputs: [["y", z.string()]] as const,
    });
    const engine = { registerBlock: vi.fn() };
    Block.register(engine as any);

    expect(engine.registerBlock).toHaveBeenCalledWith(
      expect.objectContaining({ name: "TestBlock" }),
      expect.any(Function),
    );
  });

  it("infers execute parameter types from inputs — no annotation needed", async () => {
    // 'n' is inferred as number, 's' as string — no ': [number, string]' required
    const Block = defineBlock({
      desc,
      inputs: [
        ["n", z.number()],
        ["s", z.string()],
      ] as const,
      outputs: [["out", z.boolean()]] as const,
      execute([n, s]) {
        return Promise.resolve([n > 0 && s.length > 0]);
      },
    });

    const b = new Block();
    await expect((b as any).executeImpl([3, "hi"])).resolves.toEqual([true]);
    await expect((b as any).executeImpl([-1, "hi"])).resolves.toEqual([false]);
  });
});

describe("zodToKind mapping", () => {
  const cases = [
    { schema: z.boolean(), kind: "bool" },
    { schema: z.number(), kind: "number" },
    { schema: z.string(), kind: "str" },
    { schema: z.enum(["a", "b"]), kind: "str" },
    { schema: z.array(z.string()), kind: "list" },
    { schema: z.object({ a: z.string() }), kind: "dict" },
    { schema: z.unknown(), kind: "null" },
    { schema: z.optional(z.string()), kind: "str" },
    { schema: z.string().default("x"), kind: "str" },
  ] as const;

  for (const { schema, kind } of cases) {
    it(`maps ${schema.constructor.name} to '${kind}'`, () => {
      const Block = defineBlock({
        desc,
        inputs: [["pin", schema]] as const,
        outputs: [["out", z.unknown()]] as const,
      });
      expect(new Block().desc.inputs[0].kind).toBe(kind);
    });
  }
});

describe("TypedBlock.executeImpl", () => {
  // Inline execute — types inferred, no annotation needed
  const TestBase = defineBlock({
    desc,
    inputs: [
      ["x", z.number()],
      ["label", z.string()],
    ] as const,
    outputs: [["result", z.boolean()]] as const,
    execute([x, _label]) {
      return Promise.resolve([x > 0]);
    },
  });

  class TestBlock extends TestBase {
    runImpl(inputs: unknown[]) {
      return this.executeImpl(inputs as any);
    }
  }

  it("throws when input count is wrong", async () => {
    const b = new TestBlock();
    await expect(b.runImpl([1])).rejects.toThrow("Invalid number of inputs");
  });

  it("parses inputs via Zod and forwards them to execute", async () => {
    const b = new TestBlock();
    const executeSpy = vi.spyOn(b, "execute");
    await b.runImpl([7, "hello"]);
    expect(executeSpy).toHaveBeenCalledWith([7, "hello"]);
  });

  it("returns undefined when execute returns undefined", async () => {
    class UndefBlock extends TestBase {
      override execute(_inputs: [number, string]) {
        return Promise.resolve(undefined);
      }
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    const result = await new UndefBlock().runImpl([1, "hi"]);
    expect(result).toBeUndefined();
  });

  it("throws when output count is wrong", async () => {
    class BadOutputBlock extends TestBase {
      override execute(_inputs: [number, string]) {
        return Promise.resolve([true, "extra"] as any);
      }
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    await expect(new BadOutputBlock().runImpl([1, "hi"])).rejects.toThrow(
      "Invalid number of outputs",
    );
  });

  it("returns the execute result", async () => {
    const b = new TestBlock();
    const result = await b.runImpl([5, "hello"]);
    expect(result).toEqual([true]);
  });

  it("throws when input Zod validation fails", async () => {
    const b = new TestBlock();
    await expect(b.runImpl(["not-a-number", "hello"])).rejects.toThrow();
  });
});
