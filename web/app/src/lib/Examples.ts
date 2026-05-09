import type { Program } from "logic-mesh";

// Each demo uses a deterministic UUID prefix so the IDs stay stable
// across reloads and don't collide between examples.

const datReset = {
  name: "DAT Temperature Reset",
  description:
    "Discharge-air-temperature reset (ASHRAE G36 style): as outdoor temp rises, the supply-air setpoint falls. PID drives the simulated SAT toward the SP.",
  blocks: {
    "11111111-1111-4111-8111-000000000001": {
      name: "Slider",
      lib: "ui",
      label: "OAT (°F)",
      positions: { x: 40, y: 80 },
      inputs: {
        in: { value: 60, isConnected: false },
        min: { value: 30, isConnected: false },
        max: { value: 90, isConnected: false },
        step: { value: 1, isConnected: false },
      },
      outputs: { out: { value: 60 } },
    },
    "11111111-1111-4111-8111-000000000002": {
      name: "Reset",
      lib: "core",
      label: "OAT → SAT setpoint",
      positions: { x: 270, y: 80 },
      inputs: {
        inMin: { value: 50, isConnected: false },
        inMax: { value: 70, isConnected: false },
        outMin: { value: 65, isConnected: false },
        outMax: { value: 55, isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000003": {
      name: "Pid",
      lib: "core",
      label: "SAT loop",
      positions: { x: 500, y: 80 },
      inputs: {
        interval: { value: 200, isConnected: false },
        kp: { value: 0.6, isConnected: false },
        ki: { value: 0.05, isConnected: false },
        kd: { value: 0.1, isConnected: false },
        min: { value: 50, isConnected: false },
        max: { value: 70, isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000004": {
      name: "MultiChart",
      lib: "ui",
      positions: { x: 740, y: 30 },
      inputs: {
        labelA: { value: "OAT", isConnected: false },
        labelB: { value: "SAT SP", isConnected: false },
        labelC: { value: "SAT PV", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000005": {
      name: "Display",
      lib: "ui",
      positions: { x: 40, y: 240 },
      inputs: {
        unit: { value: "°F", isConnected: false },
        label: { value: "OAT", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000006": {
      name: "Display",
      lib: "ui",
      positions: { x: 270, y: 240 },
      inputs: {
        unit: { value: "°F", isConnected: false },
        label: { value: "SAT SP", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000007": {
      name: "Display",
      lib: "ui",
      positions: { x: 500, y: 240 },
      inputs: {
        unit: { value: "°F", isConnected: false },
        label: { value: "SAT PV", isConnected: false },
      },
    },
  },
  links: {
    "11111111-1111-4111-8111-0000000000a1": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000002",
    },
    "11111111-1111-4111-8111-0000000000a2": {
      sourceBlockPinName: "out",
      targetBlockPinName: "sp",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000003",
    },
    "11111111-1111-4111-8111-0000000000a3": {
      sourceBlockPinName: "out",
      targetBlockPinName: "a",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "11111111-1111-4111-8111-0000000000a4": {
      sourceBlockPinName: "out",
      targetBlockPinName: "b",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "11111111-1111-4111-8111-0000000000a5": {
      sourceBlockPinName: "out",
      targetBlockPinName: "c",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000003",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "11111111-1111-4111-8111-0000000000a6": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000005",
    },
    "11111111-1111-4111-8111-0000000000a7": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000006",
    },
    "11111111-1111-4111-8111-0000000000a8": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000003",
      targetBlockUuid: "11111111-1111-4111-8111-000000000007",
    },
  },
} as Program;

const coolingTower = {
  name: "Cooling Tower Stage + Lead/Lag",
  description:
    "Demand → Sequencer stages 0..2 with up/down delays → LeadLag rotates which fan is lead. Press the rotate button to swap the lead.",
  blocks: {
    "22222222-2222-4222-8222-000000000001": {
      name: "Slider",
      lib: "ui",
      label: "Cooling demand 0–1",
      positions: { x: 40, y: 80 },
      inputs: {
        in: { value: 0.5, isConnected: false },
        min: { value: 0, isConnected: false },
        max: { value: 1, isConnected: false },
        step: { value: 0.1, isConnected: false },
      },
      outputs: { out: { value: 0.5 } },
    },
    "22222222-2222-4222-8222-000000000002": {
      name: "Sequencer",
      lib: "core",
      label: "2-stage stager",
      positions: { x: 270, y: 80 },
      inputs: {
        stages: { value: 2, isConnected: false },
        upDelay: { value: 2000, isConnected: false },
        downDelay: { value: 5000, isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000003": {
      name: "Button",
      lib: "ui",
      label: "Rotate lead",
      positions: { x: 40, y: 230 },
    },
    "22222222-2222-4222-8222-000000000004": {
      name: "LeadLag",
      lib: "core",
      label: "Fan A/B rotation",
      positions: { x: 500, y: 80 },
      inputs: {
        enable: { value: true, isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000005": {
      name: "Led",
      lib: "ui",
      positions: { x: 740, y: 60 },
      inputs: {
        label: { value: "Fan A", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000006": {
      name: "Led",
      lib: "ui",
      positions: { x: 740, y: 170 },
      inputs: {
        label: { value: "Fan B", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000007": {
      name: "Display",
      lib: "ui",
      positions: { x: 270, y: 240 },
      inputs: {
        unit: { value: "stages", isConnected: false },
        label: { value: "Active", isConnected: false },
      },
    },
  },
  links: {
    "22222222-2222-4222-8222-0000000000a1": {
      sourceBlockPinName: "out",
      targetBlockPinName: "demand",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000001",
      targetBlockUuid: "22222222-2222-4222-8222-000000000002",
    },
    "22222222-2222-4222-8222-0000000000a2": {
      sourceBlockPinName: "out",
      targetBlockPinName: "demand",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000002",
      targetBlockUuid: "22222222-2222-4222-8222-000000000004",
    },
    "22222222-2222-4222-8222-0000000000a3": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rotate",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000003",
      targetBlockUuid: "22222222-2222-4222-8222-000000000004",
    },
    "22222222-2222-4222-8222-0000000000a4": {
      sourceBlockPinName: "a",
      targetBlockPinName: "in",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000004",
      targetBlockUuid: "22222222-2222-4222-8222-000000000005",
    },
    "22222222-2222-4222-8222-0000000000a5": {
      sourceBlockPinName: "b",
      targetBlockPinName: "in",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000004",
      targetBlockUuid: "22222222-2222-4222-8222-000000000006",
    },
    "22222222-2222-4222-8222-0000000000a6": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000002",
      targetBlockUuid: "22222222-2222-4222-8222-000000000007",
    },
  },
} as Program;

const economizer = {
  name: "Air-Side Economizer (Enthalpy)",
  description:
    "Compares outdoor and return air enthalpy. Free cooling is available whenever h_OA < h_RA. Drag the OAT/RH/RAT sliders to see the decision flip.",
  blocks: {
    "33333333-3333-4333-8333-000000000001": {
      name: "Slider",
      lib: "ui",
      label: "OAT (°C)",
      positions: { x: 40, y: 40 },
      inputs: {
        in: { value: 18, isConnected: false },
        min: { value: -10, isConnected: false },
        max: { value: 40, isConnected: false },
        step: { value: 0.5, isConnected: false },
      },
      outputs: { out: { value: 18 } },
    },
    "33333333-3333-4333-8333-000000000002": {
      name: "Slider",
      lib: "ui",
      label: "OA RH (%)",
      positions: { x: 40, y: 130 },
      inputs: {
        in: { value: 50, isConnected: false },
        min: { value: 0, isConnected: false },
        max: { value: 100, isConnected: false },
        step: { value: 1, isConnected: false },
      },
      outputs: { out: { value: 50 } },
    },
    "33333333-3333-4333-8333-000000000003": {
      name: "Slider",
      lib: "ui",
      label: "RAT (°C)",
      positions: { x: 40, y: 230 },
      inputs: {
        in: { value: 24, isConnected: false },
        min: { value: 18, isConnected: false },
        max: { value: 30, isConnected: false },
        step: { value: 0.5, isConnected: false },
      },
      outputs: { out: { value: 24 } },
    },
    "33333333-3333-4333-8333-000000000004": {
      name: "Slider",
      lib: "ui",
      label: "RA RH (%)",
      positions: { x: 40, y: 320 },
      inputs: {
        in: { value: 50, isConnected: false },
        min: { value: 20, isConnected: false },
        max: { value: 80, isConnected: false },
        step: { value: 1, isConnected: false },
      },
      outputs: { out: { value: 50 } },
    },
    "33333333-3333-4333-8333-000000000005": {
      name: "Enthalpy",
      lib: "core",
      label: "Outdoor air",
      positions: { x: 280, y: 70 },
    },
    "33333333-3333-4333-8333-000000000006": {
      name: "Enthalpy",
      lib: "core",
      label: "Return air",
      positions: { x: 280, y: 260 },
    },
    "33333333-3333-4333-8333-000000000007": {
      name: "LessThan",
      lib: "core",
      label: "h_OA < h_RA?",
      positions: { x: 520, y: 170 },
    },
    "33333333-3333-4333-8333-000000000008": {
      name: "Led",
      lib: "ui",
      positions: { x: 740, y: 170 },
      inputs: {
        label: { value: "Free Cooling", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-000000000009": {
      name: "MultiChart",
      lib: "ui",
      positions: { x: 740, y: 30 },
      inputs: {
        labelA: { value: "h_OA", isConnected: false },
        labelB: { value: "h_RA", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-00000000000a": {
      name: "Display",
      lib: "ui",
      positions: { x: 280, y: 200 },
      inputs: {
        unit: { value: "kJ/kg", isConnected: false },
        label: { value: "h OA", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-00000000000b": {
      name: "Display",
      lib: "ui",
      positions: { x: 280, y: 380 },
      inputs: {
        unit: { value: "kJ/kg", isConnected: false },
        label: { value: "h RA", isConnected: false },
      },
    },
  },
  links: {
    "33333333-3333-4333-8333-0000000000a1": {
      sourceBlockPinName: "out",
      targetBlockPinName: "t",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000001",
      targetBlockUuid: "33333333-3333-4333-8333-000000000005",
    },
    "33333333-3333-4333-8333-0000000000a2": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rh",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000002",
      targetBlockUuid: "33333333-3333-4333-8333-000000000005",
    },
    "33333333-3333-4333-8333-0000000000a3": {
      sourceBlockPinName: "out",
      targetBlockPinName: "t",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000003",
      targetBlockUuid: "33333333-3333-4333-8333-000000000006",
    },
    "33333333-3333-4333-8333-0000000000a4": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rh",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000004",
      targetBlockUuid: "33333333-3333-4333-8333-000000000006",
    },
    "33333333-3333-4333-8333-0000000000a5": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in1",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-000000000007",
    },
    "33333333-3333-4333-8333-0000000000a6": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in2",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000006",
      targetBlockUuid: "33333333-3333-4333-8333-000000000007",
    },
    "33333333-3333-4333-8333-0000000000a7": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000007",
      targetBlockUuid: "33333333-3333-4333-8333-000000000008",
    },
    "33333333-3333-4333-8333-0000000000a8": {
      sourceBlockPinName: "out",
      targetBlockPinName: "a",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-000000000009",
    },
    "33333333-3333-4333-8333-0000000000a9": {
      sourceBlockPinName: "out",
      targetBlockPinName: "b",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000006",
      targetBlockUuid: "33333333-3333-4333-8333-000000000009",
    },
    "33333333-3333-4333-8333-0000000000aa": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-00000000000a",
    },
    "33333333-3333-4333-8333-0000000000ab": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000006",
      targetBlockUuid: "33333333-3333-4333-8333-00000000000b",
    },
  },
} as Program;

const antiShortCycle = {
  name: "Anti-Short-Cycle Compressor",
  description:
    "Toggle the call: OnDelay holds the compressor off until the call has been steady for 3 s; OffDelay keeps the cool-down lockout active for 10 s after the call drops.",
  blocks: {
    "44444444-4444-4444-8444-000000000001": {
      name: "Checkbox",
      lib: "ui",
      label: "Cooling call",
      positions: { x: 40, y: 100 },
    },
    "44444444-4444-4444-8444-000000000002": {
      name: "OnDelay",
      lib: "core",
      label: "3 s warmup",
      positions: { x: 260, y: 50 },
      inputs: {
        delay: { value: 3000, isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000003": {
      name: "OffDelay",
      lib: "core",
      label: "10 s cool-down",
      positions: { x: 260, y: 200 },
      inputs: {
        delay: { value: 10000, isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000004": {
      name: "Led",
      lib: "ui",
      positions: { x: 510, y: 50 },
      inputs: {
        label: { value: "Compressor (3s warmup)", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000005": {
      name: "Led",
      lib: "ui",
      positions: { x: 510, y: 200 },
      inputs: {
        label: { value: "Cool-down (10s)", isConnected: false },
        color: { value: "#f59e0b", isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000006": {
      name: "Display",
      lib: "ui",
      positions: { x: 40, y: 220 },
      inputs: {
        label: { value: "Call", isConnected: false },
      },
    },
  },
  links: {
    "44444444-4444-4444-8444-0000000000a1": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000002",
    },
    "44444444-4444-4444-8444-0000000000a2": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000003",
    },
    "44444444-4444-4444-8444-0000000000a3": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000002",
      targetBlockUuid: "44444444-4444-4444-8444-000000000004",
    },
    "44444444-4444-4444-8444-0000000000a4": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000003",
      targetBlockUuid: "44444444-4444-4444-8444-000000000005",
    },
    "44444444-4444-4444-8444-0000000000a5": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000006",
    },
  },
} as Program;

export const examplePrograms = [datReset, coolingTower, economizer, antiShortCycle];
