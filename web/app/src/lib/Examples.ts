import type { Program } from 'logic-mesh';

// Each demo uses a deterministic UUID prefix so the IDs stay stable
// across reloads and don't collide between examples.
//
// UI widgets are ExternalIn/ExternalOut blocks bound to the 'ui'
// connector, addressed by their own block UUID, with the widget
// identity and configuration carried in the `widget` field.

const datReset = {
  name: 'DAT Temperature Reset',
  description:
    'Discharge-air-temperature reset (ASHRAE G36 style): as outdoor temp rises, the supply-air setpoint falls. PID drives the simulated SAT toward the SP. The PV bar scales to the live SP (config drive) and the tracking slider follows the auto SP until you drag it (value feedback).',
  blocks: {
    '11111111-1111-4111-8111-000000000001': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 81, y: -72 },
      label: 'OAT (°F)',
      widget: {
        kind: 'Slider',
        config: { value: 60, min: 30, max: 90, step: 1 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000001',
          isConnected: false,
        },
      },
      outputs: { out: { value: 60 } },
    },
    '11111111-1111-4111-8111-000000000002': {
      name: 'Reset',
      lib: 'core',
      positions: { x: 267, y: 25 },
      label: 'OAT → SAT setpoint',
      inputs: {
        in: { value: 60, isConnected: false },
        inMin: { value: 50, isConnected: false },
        inMax: { value: 70, isConnected: false },
        outMin: { value: 65, isConnected: false },
        outMax: { value: 55, isConnected: false },
      },
      outputs: { out: { value: 60 } },
    },
    '11111111-1111-4111-8111-000000000003': {
      name: 'Pid',
      lib: 'core',
      positions: { x: 561, y: 107 },
      label: 'SAT loop',
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
    '11111111-1111-4111-8111-000000000004': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 1200, y: -60 },
      widget: {
        kind: 'MultiChart',
        config: {
          series: [
            { label: 'OAT' },
            {
              label: 'SAT SP',
              address: '11111111-1111-4111-8111-000000000008',
            },
            {
              label: 'SAT PV',
              address: '11111111-1111-4111-8111-000000000009',
            },
          ],
        },
      },
      inputs: {
        in: { value: 60, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000004',
          isConnected: false,
        },
      },
    },
    '11111111-1111-4111-8111-000000000005': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 244, y: -153.63 },
      widget: { kind: 'Display', config: { unit: '°F', label: 'OAT' } },
      inputs: {
        in: { value: 60, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000005',
          isConnected: false,
        },
      },
    },
    '11111111-1111-4111-8111-000000000006': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 344, y: 290 },
      widget: { kind: 'Display', config: { unit: '°F', label: 'SAT SP' } },
      inputs: {
        in: { value: 60, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000006',
          isConnected: false,
        },
      },
    },
    '11111111-1111-4111-8111-000000000007': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 736, y: 289 },
      widget: { kind: 'Display', config: { unit: '°F', label: 'SAT PV' } },
      inputs: {
        in: { value: 58.93, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000007',
          isConnected: false,
        },
      },
    },
    // Plain (non-widget) ExternalOut blocks feeding the MultiChart's
    // extra series by address.
    '11111111-1111-4111-8111-000000000008': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 960, y: -20 },
      label: 'SAT SP series',
      inputs: {
        in: { value: 60, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000008',
          isConnected: false,
        },
      },
    },
    '11111111-1111-4111-8111-000000000009': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 960, y: 150 },
      label: 'SAT PV series',
      inputs: {
        in: { value: 58.93, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-000000000009',
          isConnected: false,
        },
      },
    },
    // The bar's `max` is driven at runtime by the SAT SP published to
    // the plain ExternalOut …0008, so its scale follows the live SP.
    '11111111-1111-4111-8111-00000000000a': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 1200, y: 200 },
      widget: {
        kind: 'Bar',
        config: { min: 50, max: 70, label: 'PV vs SP' },
        configSources: { max: '11111111-1111-4111-8111-000000000008' },
      },
      inputs: {
        in: { value: 58.93, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-00000000000a',
          isConnected: false,
        },
      },
    },
    // Operator SP station: tracks the auto SP published to …0008 as
    // feedback until the user drags the slider.
    '11111111-1111-4111-8111-00000000000b': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 81, y: 60 },
      label: 'SAT SP (tracks auto)',
      widget: {
        kind: 'Slider',
        config: { value: 60, min: 50, max: 70, step: 0.5 },
        valueSource: '11111111-1111-4111-8111-000000000008',
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '11111111-1111-4111-8111-00000000000b',
          isConnected: false,
        },
      },
      outputs: { out: { value: 60 } },
    },
  },
  links: {
    'c5011201-d2e8-4b34-aac1-531ec60fc780': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000001',
      targetBlockUuid: '11111111-1111-4111-8111-000000000002',
    },
    '16b43c2e-167d-411e-aab1-09a2ab3f5412': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'sp',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000002',
      targetBlockUuid: '11111111-1111-4111-8111-000000000003',
    },
    'df1ee3bd-8b3a-4a51-9c6e-2f4b7d1a9e03': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000001',
      targetBlockUuid: '11111111-1111-4111-8111-000000000004',
    },
    '5abd9fc3-14e7-4b6d-8f20-6c1d3a5e9b47': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000002',
      targetBlockUuid: '11111111-1111-4111-8111-000000000008',
    },
    'f00793e3-d209-42de-b562-2ead9532ca5c': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000003',
      targetBlockUuid: '11111111-1111-4111-8111-000000000009',
    },
    '803cabeb-ee6a-4344-81a8-85f1d5f17fd3': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000001',
      targetBlockUuid: '11111111-1111-4111-8111-000000000005',
    },
    '244d3a6e-5e3b-4733-bbfb-c2d380a2d335': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000002',
      targetBlockUuid: '11111111-1111-4111-8111-000000000006',
    },
    '743d173b-1f1f-4e00-8eaf-dff60b4929bb': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000003',
      targetBlockUuid: '11111111-1111-4111-8111-000000000007',
    },
    '2f8c1b4d-6a3e-47f0-9d52-1e7b9c0a5d38': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '11111111-1111-4111-8111-000000000003',
      targetBlockUuid: '11111111-1111-4111-8111-00000000000a',
    },
  },
} as Program;

const coolingTower = {
  name: 'Cooling Tower Stage + Lead/Lag',
  description:
    'Demand → Sequencer stages 0..2 with up/down delays → LeadLag rotates which fan is lead. Press the rotate button to swap the lead.',
  blocks: {
    '22222222-2222-4222-8222-000000000001': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 10, y: 33 },
      label: 'Cooling demand 0–1',
      widget: {
        kind: 'Slider',
        config: { value: 0.3, min: 0, max: 1, step: 0.1 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '22222222-2222-4222-8222-000000000001',
          isConnected: false,
        },
      },
      outputs: { out: { value: 0.3 } },
    },
    '22222222-2222-4222-8222-000000000002': {
      name: 'Sequencer',
      lib: 'core',
      positions: { x: 271, y: 15 },
      label: '2-stage stager',
      inputs: {
        demand: { value: 0.3, isConnected: false },
        stages: { value: 2, isConnected: false },
        upDelay: { value: 2000, isConnected: false },
        downDelay: { value: 5000, isConnected: false },
      },
      outputs: { out: { value: 1 } },
    },
    '22222222-2222-4222-8222-000000000003': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 13, y: 151 },
      label: 'Rotate lead',
      widget: { kind: 'Button' },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '22222222-2222-4222-8222-000000000003',
          isConnected: false,
        },
      },
      outputs: { out: { value: false } },
    },
    '22222222-2222-4222-8222-000000000004': {
      name: 'LeadLag',
      lib: 'core',
      positions: { x: 522, y: 96 },
      label: 'Fan A/B rotation',
      inputs: {
        enable: { value: true, isConnected: false },
        rotate: { value: false, isConnected: false },
        demand: { value: 1, isConnected: false },
      },
      outputs: { a: { value: true }, b: { value: false } },
    },
    '22222222-2222-4222-8222-000000000005': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 740, y: 60 },
      widget: { kind: 'Led', config: { label: 'Fan A', color: '#3ecf6b' } },
      inputs: {
        in: { value: true, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '22222222-2222-4222-8222-000000000005',
          isConnected: false,
        },
      },
    },
    '22222222-2222-4222-8222-000000000006': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 740, y: 170 },
      widget: { kind: 'Led', config: { label: 'Fan B', color: '#3ecf6b' } },
      inputs: {
        in: { value: false, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '22222222-2222-4222-8222-000000000006',
          isConnected: false,
        },
      },
    },
    '22222222-2222-4222-8222-000000000007': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 510, y: -88 },
      widget: {
        kind: 'Display',
        config: { unit: 'stages', label: 'Active' },
      },
      inputs: {
        in: { value: 1, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '22222222-2222-4222-8222-000000000007',
          isConnected: false,
        },
      },
    },
  },
  links: {
    '62610e3d-b6c2-4c81-8499-b61076f623ef': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'demand',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000001',
      targetBlockUuid: '22222222-2222-4222-8222-000000000002',
    },
    '91330256-34c0-48f5-885a-c17d46c3d3d4': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'demand',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000002',
      targetBlockUuid: '22222222-2222-4222-8222-000000000004',
    },
    '6b2b7a55-9f3a-4342-9399-aefd6754ba0b': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'rotate',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000003',
      targetBlockUuid: '22222222-2222-4222-8222-000000000004',
    },
    '569ce2fa-bfd9-4a6e-9bde-f5020c1de74c': {
      sourceBlockPinName: 'a',
      targetBlockPinName: 'in',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000004',
      targetBlockUuid: '22222222-2222-4222-8222-000000000005',
    },
    '148f4a18-b4a4-43de-aa44-731bbdf4085c': {
      sourceBlockPinName: 'b',
      targetBlockPinName: 'in',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000004',
      targetBlockUuid: '22222222-2222-4222-8222-000000000006',
    },
    '9df2aeab-f2e3-4973-9347-9b49e129f28d': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '22222222-2222-4222-8222-000000000002',
      targetBlockUuid: '22222222-2222-4222-8222-000000000007',
    },
  },
} as Program;

const economizer = {
  name: 'Air-Side Economizer (Enthalpy)',
  description:
    'Compares outdoor and return air enthalpy. Free cooling is available whenever h_OA < h_RA. Drag the OAT/RH/RAT sliders to see the decision flip.',
  blocks: {
    '33333333-3333-4333-8333-000000000001': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 40, y: 40 },
      label: 'OAT (°C)',
      widget: {
        kind: 'Slider',
        config: { value: 18, min: -10, max: 40, step: 0.5 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000001',
          isConnected: false,
        },
      },
      outputs: { out: { value: 18 } },
    },
    '33333333-3333-4333-8333-000000000002': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 40, y: 130 },
      label: 'OA RH (%)',
      widget: {
        kind: 'Slider',
        config: { value: 50, min: 0, max: 100, step: 1 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000002',
          isConnected: false,
        },
      },
      outputs: { out: { value: 50 } },
    },
    '33333333-3333-4333-8333-000000000003': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 40, y: 230 },
      label: 'RAT (°C)',
      widget: {
        kind: 'Slider',
        config: { value: 24, min: 18, max: 30, step: 0.5 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000003',
          isConnected: false,
        },
      },
      outputs: { out: { value: 24 } },
    },
    '33333333-3333-4333-8333-000000000004': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 40, y: 320 },
      label: 'RA RH (%)',
      widget: {
        kind: 'Slider',
        config: { value: 50, min: 20, max: 80, step: 1 },
      },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000004',
          isConnected: false,
        },
      },
      outputs: { out: { value: 50 } },
    },
    '33333333-3333-4333-8333-000000000005': {
      name: 'Enthalpy',
      lib: 'core',
      positions: { x: 272, y: 33 },
      label: 'Outdoor air',
      inputs: {
        t: { value: 18, isConnected: false },
        rh: { value: 50, isConnected: false },
      },
      outputs: { out: { value: {} } },
    },
    '33333333-3333-4333-8333-000000000006': {
      name: 'Enthalpy',
      lib: 'core',
      positions: { x: 275, y: 236 },
      label: 'Return air',
      inputs: {
        t: { value: 24, isConnected: false },
        rh: { value: 50, isConnected: false },
      },
      outputs: { out: { value: {} } },
    },
    '33333333-3333-4333-8333-000000000007': {
      name: 'LessThan',
      lib: 'core',
      positions: { x: 673, y: 283 },
      label: 'h_OA < h_RA?',
      inputs: {
        in1: { value: {}, isConnected: false },
        in2: { value: {}, isConnected: false },
      },
      outputs: { out: { value: true } },
    },
    '33333333-3333-4333-8333-000000000008': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 935, y: 276 },
      widget: {
        kind: 'Led',
        config: { label: 'Free Cooling', color: '#3ecf6b' },
      },
      inputs: {
        in: { value: true, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000008',
          isConnected: false,
        },
      },
    },
    '33333333-3333-4333-8333-000000000009': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 737, y: 28.63 },
      widget: {
        kind: 'MultiChart',
        config: {
          series: [
            { label: 'h_OA' },
            {
              label: 'h_RA',
              address: '33333333-3333-4333-8333-00000000000c',
            },
          ],
        },
      },
      inputs: {
        in: { value: {}, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-000000000009',
          isConnected: false,
        },
      },
    },
    '33333333-3333-4333-8333-00000000000a': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 480, y: -79.25 },
      widget: { kind: 'Display', config: { unit: 'kJ/kg', label: 'h OA' } },
      inputs: {
        in: { value: {}, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-00000000000a',
          isConnected: false,
        },
      },
    },
    '33333333-3333-4333-8333-00000000000b': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 484, y: 454 },
      widget: { kind: 'Display', config: { unit: 'kJ/kg', label: 'h RA' } },
      inputs: {
        in: { value: {}, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-00000000000b',
          isConnected: false,
        },
      },
    },
    // Plain (non-widget) ExternalOut feeding the MultiChart's h_RA
    // series by address.
    '33333333-3333-4333-8333-00000000000c': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 480, y: 140 },
      label: 'h_RA series',
      inputs: {
        in: { value: {}, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '33333333-3333-4333-8333-00000000000c',
          isConnected: false,
        },
      },
    },
  },
  links: {
    'a5364c93-9b71-43d0-a40c-5f10f652ed95': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 't',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000001',
      targetBlockUuid: '33333333-3333-4333-8333-000000000005',
    },
    'a41d3375-877a-40dc-9b1a-ff01a123c2ca': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'rh',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000002',
      targetBlockUuid: '33333333-3333-4333-8333-000000000005',
    },
    '65f62d1e-a817-4d68-a243-bdc300d26f54': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 't',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000003',
      targetBlockUuid: '33333333-3333-4333-8333-000000000006',
    },
    '900cdf70-a99a-4225-a27e-daf80c74cf65': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'rh',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000004',
      targetBlockUuid: '33333333-3333-4333-8333-000000000006',
    },
    '56f5471c-0ed8-4aa6-8233-6f93bfa4f5aa': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in1',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000005',
      targetBlockUuid: '33333333-3333-4333-8333-000000000007',
    },
    '43dd66d5-d064-4d80-93bf-744b246ad534': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in2',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000006',
      targetBlockUuid: '33333333-3333-4333-8333-000000000007',
    },
    '8c3a72be-8e51-420d-a385-446b1106b857': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000007',
      targetBlockUuid: '33333333-3333-4333-8333-000000000008',
    },
    'f8374721-a736-4c33-afcf-a3fdf0d9efef': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000005',
      targetBlockUuid: '33333333-3333-4333-8333-000000000009',
    },
    'b6a98b22-962e-4423-99ea-8d180fa05f06': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000005',
      targetBlockUuid: '33333333-3333-4333-8333-00000000000a',
    },
    'a09aed12-6901-406d-87ff-de156ec59785': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000006',
      targetBlockUuid: '33333333-3333-4333-8333-00000000000b',
    },
    'c0478a50-2d91-4e3f-8b7a-5f6c1d2e3a4b': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '33333333-3333-4333-8333-000000000006',
      targetBlockUuid: '33333333-3333-4333-8333-00000000000c',
    },
  },
} as Program;

const antiShortCycle = {
  name: 'Anti-Short-Cycle Compressor',
  description:
    'Toggle the call: OnDelay holds the compressor off until the call has been steady for 3 s; OffDelay keeps the cool-down lockout active for 10 s after the call drops.',
  blocks: {
    '44444444-4444-4444-8444-000000000001': {
      name: 'ExternalIn',
      lib: 'core',
      positions: { x: 18, y: 32 },
      label: 'Cooling call',
      widget: { kind: 'Checkbox', config: { value: false } },
      inputs: {
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '44444444-4444-4444-8444-000000000001',
          isConnected: false,
        },
      },
    },
    '44444444-4444-4444-8444-000000000002': {
      name: 'OnDelay',
      lib: 'core',
      positions: { x: 266, y: 12 },
      label: '3 s warmup',
      inputs: {
        in: { value: false, isConnected: false },
        delay: { value: 3000, isConnected: false },
      },
      outputs: { out: { value: false } },
    },
    '44444444-4444-4444-8444-000000000003': {
      name: 'OffDelay',
      lib: 'core',
      positions: { x: 268, y: 145 },
      label: '10 s cool-down',
      inputs: {
        in: { value: false, isConnected: false },
        delay: { value: 10000, isConnected: false },
      },
      outputs: { out: { value: true } },
    },
    '44444444-4444-4444-8444-000000000004': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 505, y: 6 },
      widget: {
        kind: 'Led',
        config: { label: 'Compressor (3s warmup)', color: '#3ecf6b' },
      },
      inputs: {
        in: { value: false, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '44444444-4444-4444-8444-000000000004',
          isConnected: false,
        },
      },
    },
    '44444444-4444-4444-8444-000000000005': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 505, y: 139 },
      widget: {
        kind: 'Led',
        config: { label: 'Cool-down (10s)', color: '#f59e0b' },
      },
      inputs: {
        in: { value: true, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '44444444-4444-4444-8444-000000000005',
          isConnected: false,
        },
      },
    },
    '44444444-4444-4444-8444-000000000006': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 265, y: -118 },
      widget: { kind: 'Display', config: { label: 'Call' } },
      inputs: {
        in: { value: 0, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '44444444-4444-4444-8444-000000000006',
          isConnected: false,
        },
      },
    },
  },
  links: {
    '1e585ee8-310c-4837-9c3a-eb6879e86007': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '44444444-4444-4444-8444-000000000001',
      targetBlockUuid: '44444444-4444-4444-8444-000000000002',
    },
    '3c0d484a-55cb-452d-85fd-f57cc51b40a5': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '44444444-4444-4444-8444-000000000001',
      targetBlockUuid: '44444444-4444-4444-8444-000000000003',
    },
    'fdf7c29e-b0d2-476a-aacc-ed8b4a0bd49c': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '44444444-4444-4444-8444-000000000002',
      targetBlockUuid: '44444444-4444-4444-8444-000000000004',
    },
    'a052bc6d-fb71-42a5-b7c8-8e3bf3b1cacb': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '44444444-4444-4444-8444-000000000003',
      targetBlockUuid: '44444444-4444-4444-8444-000000000005',
    },
    '794a4cbf-2c54-42f2-9a5f-6bee5118aa18': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '44444444-4444-4444-8444-000000000001',
      targetBlockUuid: '44444444-4444-4444-8444-000000000006',
    },
  },
} as Program;

const outdoorLighting = {
  name: 'Outdoor Lighting (dusk-to-cutoff)',
  description:
    "Streetlight-style lighting: turns on at sunset (Sun.isDay = false) AND while a Schedule says we're in the active window (16:00–23:30 daily). Edit the lat/lon, tzOffset, and schedule to relocate.",
  blocks: {
    '55555555-5555-4555-8555-000000000001': {
      name: 'Sun',
      lib: 'core',
      positions: { x: 36, y: 94 },
      label: 'NYC sun position',
      inputs: {
        lat: { value: 40.71, isConnected: false },
        lon: { value: -74.01, isConnected: false },
        tzOffset: { value: -300, isConnected: false },
      },
      outputs: {
        sunrise: { value: 441.12 },
        sunset: { value: 999.61 },
        isDay: { value: false },
      },
    },
    '55555555-5555-4555-8555-000000000002': {
      name: 'Schedule',
      lib: 'core',
      positions: { x: 312, y: 343 },
      label: 'Active hours',
      inputs: {
        start: { value: '16:00', isConnected: false },
        end: { value: '23:30', isConnected: false },
        days: { value: 'MTWRFSU', isConnected: false },
        tzOffset: { value: -300, isConnected: false },
      },
      outputs: { occupied: { value: true } },
    },
    '55555555-5555-4555-8555-000000000003': {
      name: 'Not',
      lib: 'core',
      positions: { x: 308, y: 85 },
      label: 'isNight',
      inputs: { in: { value: false, isConnected: false } },
      outputs: { out: { value: true } },
    },
    '55555555-5555-4555-8555-000000000004': {
      name: 'And',
      lib: 'core',
      positions: { x: 516, y: 115 },
      label: 'Lights enable',
      inputs: {
        in1: { value: true, isConnected: false },
        in2: { value: true, isConnected: false },
      },
      outputs: { out: { value: true } },
    },
    '55555555-5555-4555-8555-000000000005': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 729, y: 110 },
      widget: {
        kind: 'Led',
        config: { label: 'Streetlight', color: '#f59e0b' },
      },
      inputs: {
        in: { value: true, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '55555555-5555-4555-8555-000000000005',
          isConnected: false,
        },
      },
    },
    '55555555-5555-4555-8555-000000000006': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 37, y: -83 },
      widget: { kind: 'Display', config: { unit: 'min', label: 'Sunrise' } },
      inputs: {
        in: { value: 441.12, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '55555555-5555-4555-8555-000000000006',
          isConnected: false,
        },
      },
    },
    '55555555-5555-4555-8555-000000000007': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 263, y: -87 },
      widget: { kind: 'Display', config: { unit: 'min', label: 'Sunset' } },
      inputs: {
        in: { value: 999.61, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '55555555-5555-4555-8555-000000000007',
          isConnected: false,
        },
      },
    },
    '55555555-5555-4555-8555-000000000008': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 307, y: 203 },
      widget: {
        kind: 'Led',
        config: { label: 'Daytime', color: '#6b9eff' },
      },
      inputs: {
        in: { value: false, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '55555555-5555-4555-8555-000000000008',
          isConnected: false,
        },
      },
    },
    '55555555-5555-4555-8555-000000000009': {
      name: 'ExternalOut',
      lib: 'core',
      positions: { x: 604, y: 338 },
      widget: {
        kind: 'Led',
        config: { label: 'Schedule active', color: '#3ecf6b' },
      },
      inputs: {
        in: { value: true, isConnected: false },
        connector: { value: 'ui', isConnected: false },
        address: {
          value: '55555555-5555-4555-8555-000000000009',
          isConnected: false,
        },
      },
    },
  },
  links: {
    'f4c45097-219a-4b7e-874c-b217e1ea1609': {
      sourceBlockPinName: 'isDay',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000001',
      targetBlockUuid: '55555555-5555-4555-8555-000000000003',
    },
    'd397b843-743e-4a8c-9638-b5ab9bfd835b': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in1',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000003',
      targetBlockUuid: '55555555-5555-4555-8555-000000000004',
    },
    'af3b1d2a-18be-45e1-931b-493fa7444656': {
      sourceBlockPinName: 'occupied',
      targetBlockPinName: 'in2',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000002',
      targetBlockUuid: '55555555-5555-4555-8555-000000000004',
    },
    '79474e2a-3994-433b-af87-a5ea17c492c0': {
      sourceBlockPinName: 'out',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000004',
      targetBlockUuid: '55555555-5555-4555-8555-000000000005',
    },
    'b92b3cb4-bb69-493a-9bae-462396712461': {
      sourceBlockPinName: 'sunrise',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000001',
      targetBlockUuid: '55555555-5555-4555-8555-000000000006',
    },
    'a139d8a7-e2a3-4b23-ba76-cddf0c924c1f': {
      sourceBlockPinName: 'sunset',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000001',
      targetBlockUuid: '55555555-5555-4555-8555-000000000007',
    },
    '592dd10e-5d4b-4826-8e68-e4345068890a': {
      sourceBlockPinName: 'isDay',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000001',
      targetBlockUuid: '55555555-5555-4555-8555-000000000008',
    },
    '0a08fad5-7159-4bf3-87aa-99c0657147f3': {
      sourceBlockPinName: 'occupied',
      targetBlockPinName: 'in',
      sourceBlockUuid: '55555555-5555-4555-8555-000000000002',
      targetBlockUuid: '55555555-5555-4555-8555-000000000009',
    },
  },
} as Program;

export const examplePrograms = [
  datReset,
  coolingTower,
  economizer,
  antiShortCycle,
  outdoorLighting,
];
