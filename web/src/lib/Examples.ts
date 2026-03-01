import type { Program } from "logic-mesh";

const pidLoop = {
  name: "PID Loop",
  blocks: {
    "46505659-8026-4de8-98dd-d2172d57bc1b": {
      name: "Pid",
      lib: "core",
      positions: { x: 536, y: 64 },
      inputs: {
        interval: { value: 100, isConnected: true },
        sp: { value: 88, isConnected: true },
      },
      outputs: { out: { value: 70.44131757540165 } },
    },
    "10f282d1-2f80-456a-a6a2-8cddb291822f": {
      name: "Input",
      lib: "ui",
      positions: { x: 221, y: 207 },
      outputs: { out: { value: "88" } },
    },
    "88e3fcca-4949-493d-a2ba-d3a6f817805f": {
      name: "Chart",
      lib: "ui",
      positions: { x: 770, y: 36 },
      inputs: { in: { value: "70.44131757540165", isConnected: true } },
    },
    "20b00b27-e25a-44d3-906c-0c4e1abde715": {
      name: "Input",
      lib: "ui",
      positions: { x: 217, y: 103 },
      outputs: { out: { value: "100" } },
    },
  },
  links: {
    "586a2bef-fe2c-4eca-9aaf-0010b56ec385": {
      sourceBlockPinName: "out",
      targetBlockPinName: "sp",
      sourceBlockUuid: "10f282d1-2f80-456a-a6a2-8cddb291822f",
      targetBlockUuid: "46505659-8026-4de8-98dd-d2172d57bc1b",
    },
    "58f596d4-65db-4037-bfbd-7c8495f74c69": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "46505659-8026-4de8-98dd-d2172d57bc1b",
      targetBlockUuid: "88e3fcca-4949-493d-a2ba-d3a6f817805f",
    },
    "c033f434-59fa-4bbb-b4fd-1b1655426565": {
      sourceBlockPinName: "out",
      targetBlockPinName: "interval",
      sourceBlockUuid: "20b00b27-e25a-44d3-906c-0c4e1abde715",
      targetBlockUuid: "46505659-8026-4de8-98dd-d2172d57bc1b",
    },
  },
} as Program;

const sineWave = {
  name: "Sine Wave",
  blocks: {
    "28549524-491a-494a-8653-2e2b98beda10": {
      name: "Chart",
      lib: "ui",
      positions: { x: 710, y: 34 },
      inputs: { in: { value: "3.9928536847023506", isConnected: true } },
    },
    "ef7018d5-977f-402d-b5c9-e3fba2bd0955": {
      name: "Input",
      lib: "ui",
      positions: { x: 112, y: 160 },
      inputs: { in: { value: "4", isConnected: false } },
      outputs: { out: { value: "4" } },
    },
    "09bc3b7e-89cb-4cda-b5b9-a1e9512f3c79": {
      name: "Checkbox",
      lib: "ui",
      positions: { x: 822, y: 302 },
      inputs: { in: { value: true, isConnected: true } },
      outputs: { out: { value: true } },
    },
    "6e494e46-420f-4687-8f7a-c5382c4f219e": {
      name: "GreaterThan",
      lib: "core",
      positions: { x: 604, y: 320 },
      inputs: {
        in1: { value: 3.99, isConnected: true },
        in2: { value: "1.5", isConnected: true },
      },
      outputs: { out: { value: true } },
    },
    "a162e4d6-e24c-4f58-9640-ecbf142d4bb8": {
      name: "SineWave",
      lib: "core",
      positions: { x: 395, y: 76 },
      inputs: {
        freq: { value: 8, isConnected: true },
        amplitude: { value: 4, isConnected: true },
      },
      outputs: { out: { value: 3.99 } },
    },
    "dd9367ba-6269-49ac-ab2c-063c8f1adce6": {
      name: "Input",
      lib: "ui",
      positions: { x: 120, y: 15.13 },
      inputs: { in: { value: "8", isConnected: false } },
      outputs: { out: { value: "8" } },
    },
    "59d7008c-3f9b-4a98-9ea3-ee0ee7637d18": {
      name: "Input",
      lib: "ui",
      positions: { x: 343, y: 314 },
      inputs: { in: { value: "1.5", isConnected: false } },
      outputs: { out: { value: "1.5" } },
    },
  },
  links: {
    "756260e7-d5c7-4d65-859b-24e32854937c": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "6e494e46-420f-4687-8f7a-c5382c4f219e",
      targetBlockUuid: "09bc3b7e-89cb-4cda-b5b9-a1e9512f3c79",
    },
    "ca7eee20-2f28-4a0e-b2e4-357df3d73073": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "a162e4d6-e24c-4f58-9640-ecbf142d4bb8",
      targetBlockUuid: "28549524-491a-494a-8653-2e2b98beda10",
    },
    "88c47e6a-efc7-4dd7-a4f3-ede7627e238e": {
      sourceBlockPinName: "out",
      targetBlockPinName: "amplitude",
      sourceBlockUuid: "ef7018d5-977f-402d-b5c9-e3fba2bd0955",
      targetBlockUuid: "a162e4d6-e24c-4f58-9640-ecbf142d4bb8",
    },
    "317bf8c5-d32c-46f2-aa36-d1ffb7402d91": {
      sourceBlockPinName: "out",
      targetBlockPinName: "freq",
      sourceBlockUuid: "dd9367ba-6269-49ac-ab2c-063c8f1adce6",
      targetBlockUuid: "a162e4d6-e24c-4f58-9640-ecbf142d4bb8",
    },
    "e92a22f5-dcdc-4c91-8a4e-cfd0c265bf06": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in1",
      sourceBlockUuid: "a162e4d6-e24c-4f58-9640-ecbf142d4bb8",
      targetBlockUuid: "6e494e46-420f-4687-8f7a-c5382c4f219e",
    },
    "4119226e-8c44-460d-9318-85e261ee16e6": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in2",
      sourceBlockUuid: "59d7008c-3f9b-4a98-9ea3-ee0ee7637d18",
      targetBlockUuid: "6e494e46-420f-4687-8f7a-c5382c4f219e",
    },
  },
} as Program;

export const examplePrograms = [pidLoop, sineWave];
