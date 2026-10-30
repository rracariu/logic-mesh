// The sources a widget reads besides its own address. They are `ui`
// connector addresses rather than links, so these helpers are the only
// thing that tells the editor what feeds a widget.
import type { BlockDesc } from 'logic-mesh';
import { describe, expect, it } from 'vitest';
import type { Block } from './Block';
import { blockDisplayName, findPublisher, widgetSources } from './Sources';

const OWN = '00000000-0000-4000-8000-000000000001';
const SP = '00000000-0000-4000-8000-000000000008';
const PV = '00000000-0000-4000-8000-000000000009';

function block(
  name: string,
  inputs: Record<string, unknown>,
  extra: Partial<Block> = {},
): Block {
  return {
    id: crypto.randomUUID(),
    desc: { name } as BlockDesc,
    label: '',
    inputs: Object.fromEntries(
      Object.entries(inputs).map(([pin, value]) => [
        pin,
        { name: pin, kind: 'str', value, isConnected: false },
      ]),
    ),
    outputs: {},
    state: 'running',
    ...extra,
  } as Block;
}

describe('widgetSources', () => {
  it('lists the value source, then the config sources', () => {
    const sources = widgetSources(
      {
        kind: 'Slider',
        valueSource: SP,
        configSources: { max: PV, min: SP },
      },
      OWN,
    );
    expect(sources).toEqual([
      { role: 'value', address: SP },
      { role: 'max', address: PV },
      { role: 'min', address: SP },
    ]);
  });

  it("names a MultiChart's series by label, or else by slot", () => {
    const sources = widgetSources(
      {
        kind: 'MultiChart',
        config: {
          series: [
            { label: 'OAT' },
            { label: 'SAT SP', address: SP },
            { address: PV },
            null,
          ],
        },
      },
      OWN,
    );
    expect(sources).toEqual([
      { role: 'SAT SP', address: SP },
      { role: 'series 3', address: PV },
    ]);
  });

  it('reads series only on a MultiChart', () => {
    const widget = { kind: 'Chart', config: { series: [{ address: SP }] } };
    expect(widgetSources(widget, OWN)).toEqual([]);
  });

  it("skips the widget's own address and empty ones", () => {
    const sources = widgetSources(
      {
        kind: 'MultiChart',
        valueSource: '',
        configSources: { max: OWN },
        config: { series: [{ label: 'own', address: OWN }] },
      },
      OWN,
    );
    expect(sources).toEqual([]);
  });

  it('has none for a block that is not a widget', () => {
    expect(widgetSources(undefined, OWN)).toEqual([]);
  });
});

describe('findPublisher', () => {
  it('finds the ExternalOut publishing to the address on ui', () => {
    const publisher = block('ExternalOut', { connector: 'ui', address: SP });
    const blocks = [
      block('ExternalIn', { connector: 'ui', address: SP }),
      block('ExternalOut', { connector: 'mqtt', address: SP }),
      block('ExternalOut', { connector: 'ui', address: PV }),
      publisher,
    ];
    expect(findPublisher(SP, blocks)).toBe(publisher);
  });

  it('finds nothing when no block publishes to the address', () => {
    const blocks = [block('ExternalOut', { connector: 'ui', address: PV })];
    expect(findPublisher(SP, blocks)).toBeUndefined();
  });
});

describe('blockDisplayName', () => {
  it('prefers the label, then the widget config label, then the kind', () => {
    const plain = block('ExternalOut', {});
    expect(blockDisplayName({ ...plain, label: 'SAT SP series' })).toBe(
      'SAT SP series',
    );
    expect(
      blockDisplayName({
        ...plain,
        widget: { kind: 'Display', config: { label: 'SAT SP' } },
      }),
    ).toBe('SAT SP');
    expect(blockDisplayName({ ...plain, widget: { kind: 'Display' } })).toBe(
      'Display',
    );
    expect(blockDisplayName(plain)).toBe('ExternalOut');
  });
});
