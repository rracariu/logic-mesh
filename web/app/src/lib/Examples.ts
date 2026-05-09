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
      positions: { x: 81, y: -72 },
      label: "OAT (°F)",
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
      positions: { x: 267, y: 25 },
      label: "OAT → SAT setpoint",
      inputs: {
        in: { value: 60, isConnected: false },
        inMin: { value: 50, isConnected: false },
        inMax: { value: 70, isConnected: false },
        outMin: { value: 65, isConnected: false },
        outMax: { value: 55, isConnected: false },
      },
      outputs: { out: { value: 60 } },
    },
    "11111111-1111-4111-8111-000000000003": {
      name: "Pid",
      lib: "core",
      positions: { x: 561, y: 107 },
      label: "SAT loop",
      inputs: {
        sp: { value: 60, isConnected: false },
        kp: { value: 0.6, isConnected: false },
        ki: { value: 0.05, isConnected: false },
        kd: { value: 0.1, isConnected: false },
        interval: { value: 200, isConnected: false },
        min: { value: 50, isConnected: false },
        max: { value: 70, isConnected: false },
      },
      outputs: { out: { value: 58.93 } },
    },
    "11111111-1111-4111-8111-000000000004": {
      name: "MultiChart",
      lib: "ui",
      positions: { x: 749, y: -69 },
      inputs: {
        a: { value: 60, isConnected: false },
        b: { value: 60, isConnected: false },
        c: { value: 58.93, isConnected: false },
        labelA: { value: "OAT", isConnected: false },
        labelB: { value: "SAT SP", isConnected: false },
        labelC: { value: "SAT PV", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000005": {
      name: "Display",
      lib: "ui",
      positions: { x: 244, y: -153.63 },
      inputs: {
        in: { value: 60, isConnected: false },
        unit: { value: "°F", isConnected: false },
        label: { value: "OAT", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000006": {
      name: "Display",
      lib: "ui",
      positions: { x: 344, y: 290 },
      inputs: {
        in: { value: 60, isConnected: false },
        unit: { value: "°F", isConnected: false },
        label: { value: "SAT SP", isConnected: false },
      },
    },
    "11111111-1111-4111-8111-000000000007": {
      name: "Display",
      lib: "ui",
      positions: { x: 736, y: 289 },
      inputs: {
        in: { value: 58.93, isConnected: false },
        unit: { value: "°F", isConnected: false },
        label: { value: "SAT PV", isConnected: false },
      },
    },
  },
  links: {
    "c5011201-d2e8-4b34-aac1-531ec60fc780": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000002",
    },
    "16b43c2e-167d-411e-aab1-09a2ab3f5412": {
      sourceBlockPinName: "out",
      targetBlockPinName: "sp",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000003",
    },
    "df1ee3bd-8f1f-40d0-ad13-7f43165253a8": {
      sourceBlockPinName: "out",
      targetBlockPinName: "a",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "5abd9fc3-76f2-459d-89b0-d7ece94260f1": {
      sourceBlockPinName: "out",
      targetBlockPinName: "b",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "f00793e3-d209-42de-b562-2ead9532ca5c": {
      sourceBlockPinName: "out",
      targetBlockPinName: "c",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000003",
      targetBlockUuid: "11111111-1111-4111-8111-000000000004",
    },
    "803cabeb-ee6a-4344-81a8-85f1d5f17fd3": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000001",
      targetBlockUuid: "11111111-1111-4111-8111-000000000005",
    },
    "244d3a6e-5e3b-4733-bbfb-c2d380a2d335": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "11111111-1111-4111-8111-000000000002",
      targetBlockUuid: "11111111-1111-4111-8111-000000000006",
    },
    "743d173b-1f1f-4e00-8eaf-dff60b4929bb": {
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
      positions: { x: 10, y: 33 },
      label: "Cooling demand 0–1",
      inputs: {
        in: { value: 0.3, isConnected: false },
        min: { value: 0, isConnected: false },
        max: { value: 1, isConnected: false },
        step: { value: 0.1, isConnected: false },
      },
      outputs: { out: { value: 0.3 } },
    },
    "22222222-2222-4222-8222-000000000002": {
      name: "Sequencer",
      lib: "core",
      positions: { x: 271, y: 15 },
      label: "2-stage stager",
      inputs: {
        demand: { value: 0.3, isConnected: false },
        stages: { value: 2, isConnected: false },
        upDelay: { value: 2000, isConnected: false },
        downDelay: { value: 5000, isConnected: false },
      },
      outputs: { out: { value: 1 } },
    },
    "22222222-2222-4222-8222-000000000003": {
      name: "Button",
      lib: "ui",
      positions: { x: 13, y: 151 },
      label: "Rotate lead",
      outputs: { out: { value: false } },
    },
    "22222222-2222-4222-8222-000000000004": {
      name: "LeadLag",
      lib: "core",
      positions: { x: 522, y: 96 },
      label: "Fan A/B rotation",
      inputs: {
        enable: { value: true, isConnected: false },
        rotate: { value: false, isConnected: false },
        demand: { value: 1, isConnected: false },
      },
      outputs: { a: { value: true }, b: { value: false } },
    },
    "22222222-2222-4222-8222-000000000005": {
      name: "Led",
      lib: "ui",
      positions: { x: 740, y: 60 },
      inputs: {
        in: { value: true, isConnected: false },
        label: { value: "Fan A", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000006": {
      name: "Led",
      lib: "ui",
      positions: { x: 740, y: 170 },
      inputs: {
        in: { value: false, isConnected: false },
        label: { value: "Fan B", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "22222222-2222-4222-8222-000000000007": {
      name: "Display",
      lib: "ui",
      positions: { x: 510, y: -88 },
      inputs: {
        in: { value: 1, isConnected: false },
        unit: { value: "stages", isConnected: false },
        label: { value: "Active", isConnected: false },
      },
    },
  },
  links: {
    "62610e3d-b6c2-4c81-8499-b61076f623ef": {
      sourceBlockPinName: "out",
      targetBlockPinName: "demand",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000001",
      targetBlockUuid: "22222222-2222-4222-8222-000000000002",
    },
    "91330256-34c0-48f5-885a-c17d46c3d3d4": {
      sourceBlockPinName: "out",
      targetBlockPinName: "demand",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000002",
      targetBlockUuid: "22222222-2222-4222-8222-000000000004",
    },
    "6b2b7a55-9f3a-4342-9399-aefd6754ba0b": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rotate",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000003",
      targetBlockUuid: "22222222-2222-4222-8222-000000000004",
    },
    "569ce2fa-bfd9-4a6e-9bde-f5020c1de74c": {
      sourceBlockPinName: "a",
      targetBlockPinName: "in",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000004",
      targetBlockUuid: "22222222-2222-4222-8222-000000000005",
    },
    "148f4a18-b4a4-43de-aa44-731bbdf4085c": {
      sourceBlockPinName: "b",
      targetBlockPinName: "in",
      sourceBlockUuid: "22222222-2222-4222-8222-000000000004",
      targetBlockUuid: "22222222-2222-4222-8222-000000000006",
    },
    "9df2aeab-f2e3-4973-9347-9b49e129f28d": {
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
      positions: { x: 40, y: 40 },
      label: "OAT (°C)",
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
      positions: { x: 40, y: 130 },
      label: "OA RH (%)",
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
      positions: { x: 40, y: 230 },
      label: "RAT (°C)",
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
      positions: { x: 40, y: 320 },
      label: "RA RH (%)",
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
      positions: { x: 272, y: 33 },
      label: "Outdoor air",
      inputs: {
        t: { value: 18, isConnected: false },
        rh: { value: 50, isConnected: false },
      },
      outputs: { out: { value: {} } },
    },
    "33333333-3333-4333-8333-000000000006": {
      name: "Enthalpy",
      lib: "core",
      positions: { x: 275, y: 236 },
      label: "Return air",
      inputs: {
        t: { value: 24, isConnected: false },
        rh: { value: 50, isConnected: false },
      },
      outputs: { out: { value: {} } },
    },
    "33333333-3333-4333-8333-000000000007": {
      name: "LessThan",
      lib: "core",
      positions: { x: 673, y: 283 },
      label: "h_OA < h_RA?",
      inputs: {
        in1: { value: {}, isConnected: false },
        in2: { value: {}, isConnected: false },
      },
      outputs: { out: { value: true } },
    },
    "33333333-3333-4333-8333-000000000008": {
      name: "Led",
      lib: "ui",
      positions: { x: 935, y: 276 },
      inputs: {
        in: { value: true, isConnected: false },
        label: { value: "Free Cooling", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-000000000009": {
      name: "MultiChart",
      lib: "ui",
      positions: { x: 737, y: 28.63 },
      inputs: {
        a: { value: {}, isConnected: false },
        b: { value: {}, isConnected: false },
        labelA: { value: "h_OA", isConnected: false },
        labelB: { value: "h_RA", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-00000000000a": {
      name: "Display",
      lib: "ui",
      positions: { x: 480, y: -79.25 },
      inputs: {
        in: { value: {}, isConnected: false },
        unit: { value: "kJ/kg", isConnected: false },
        label: { value: "h OA", isConnected: false },
      },
    },
    "33333333-3333-4333-8333-00000000000b": {
      name: "Display",
      lib: "ui",
      positions: { x: 484, y: 454 },
      inputs: {
        in: { value: {}, isConnected: false },
        unit: { value: "kJ/kg", isConnected: false },
        label: { value: "h RA", isConnected: false },
      },
    },
  },
  links: {
    "a5364c93-9b71-43d0-a40c-5f10f652ed95": {
      sourceBlockPinName: "out",
      targetBlockPinName: "t",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000001",
      targetBlockUuid: "33333333-3333-4333-8333-000000000005",
    },
    "a41d3375-877a-40dc-9b1a-ff01a123c2ca": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rh",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000002",
      targetBlockUuid: "33333333-3333-4333-8333-000000000005",
    },
    "65f62d1e-a817-4d68-a243-bdc300d26f54": {
      sourceBlockPinName: "out",
      targetBlockPinName: "t",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000003",
      targetBlockUuid: "33333333-3333-4333-8333-000000000006",
    },
    "900cdf70-a99a-4225-a27e-daf80c74cf65": {
      sourceBlockPinName: "out",
      targetBlockPinName: "rh",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000004",
      targetBlockUuid: "33333333-3333-4333-8333-000000000006",
    },
    "56f5471c-0ed8-4aa6-8233-6f93bfa4f5aa": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in1",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-000000000007",
    },
    "43dd66d5-d064-4d80-93bf-744b246ad534": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in2",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000006",
      targetBlockUuid: "33333333-3333-4333-8333-000000000007",
    },
    "8c3a72be-8e51-420d-a385-446b1106b857": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000007",
      targetBlockUuid: "33333333-3333-4333-8333-000000000008",
    },
    "f8374721-a736-4c33-afcf-a3fdf0d9efef": {
      sourceBlockPinName: "out",
      targetBlockPinName: "a",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-000000000009",
    },
    "c0478a50-fe77-407c-8853-dec1993ccd44": {
      sourceBlockPinName: "out",
      targetBlockPinName: "b",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000006",
      targetBlockUuid: "33333333-3333-4333-8333-000000000009",
    },
    "b6a98b22-962e-4423-99ea-8d180fa05f06": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "33333333-3333-4333-8333-000000000005",
      targetBlockUuid: "33333333-3333-4333-8333-00000000000a",
    },
    "a09aed12-6901-406d-87ff-de156ec59785": {
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
      positions: { x: 18, y: 32 },
      label: "Cooling call",
    },
    "44444444-4444-4444-8444-000000000002": {
      name: "OnDelay",
      lib: "core",
      positions: { x: 266, y: 12 },
      label: "3 s warmup",
      inputs: {
        in: { value: false, isConnected: false },
        delay: { value: 3000, isConnected: false },
      },
      outputs: { out: { value: false } },
    },
    "44444444-4444-4444-8444-000000000003": {
      name: "OffDelay",
      lib: "core",
      positions: { x: 268, y: 145 },
      label: "10 s cool-down",
      inputs: {
        in: { value: false, isConnected: false },
        delay: { value: 10000, isConnected: false },
      },
      outputs: { out: { value: true } },
    },
    "44444444-4444-4444-8444-000000000004": {
      name: "Led",
      lib: "ui",
      positions: { x: 505, y: 6 },
      inputs: {
        in: { value: false, isConnected: false },
        label: { value: "Compressor (3s warmup)", isConnected: false },
        color: { value: "#3ecf6b", isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000005": {
      name: "Led",
      lib: "ui",
      positions: { x: 505, y: 139 },
      inputs: {
        in: { value: true, isConnected: false },
        label: { value: "Cool-down (10s)", isConnected: false },
        color: { value: "#f59e0b", isConnected: false },
      },
    },
    "44444444-4444-4444-8444-000000000006": {
      name: "Display",
      lib: "ui",
      positions: { x: 265, y: -118 },
      inputs: {
        in: { value: 0, isConnected: false },
        label: { value: "Call", isConnected: false },
      },
    },
  },
  links: {
    "1e585ee8-310c-4837-9c3a-eb6879e86007": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000002",
    },
    "3c0d484a-55cb-452d-85fd-f57cc51b40a5": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000003",
    },
    "fdf7c29e-b0d2-476a-aacc-ed8b4a0bd49c": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000002",
      targetBlockUuid: "44444444-4444-4444-8444-000000000004",
    },
    "a052bc6d-fb71-42a5-b7c8-8e3bf3b1cacb": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000003",
      targetBlockUuid: "44444444-4444-4444-8444-000000000005",
    },
    "794a4cbf-2c54-42f2-9a5f-6bee5118aa18": {
      sourceBlockPinName: "out",
      targetBlockPinName: "in",
      sourceBlockUuid: "44444444-4444-4444-8444-000000000001",
      targetBlockUuid: "44444444-4444-4444-8444-000000000006",
    },
  },
} as Program;

export const examplePrograms = [
  datReset,
  coolingTower,
  economizer,
  antiShortCycle,
];
