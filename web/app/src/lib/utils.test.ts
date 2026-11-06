// Reading the numbers the engine hands to JavaScript: plain numbers,
// unit-bearing Haystack numbers, and the non-finite ones, which Hayson
// encodes with a string `val`.
import { describe, expect, it } from 'vitest';
import { formatValue, isHaystackNumber, numericValue, unitOf } from './utils';

describe('Haystack numbers', () => {
  it('read a unit-bearing number', () => {
    const v = { _kind: 'number', val: 47.4, unit: 'kJ/kg' };
    expect(isHaystackNumber(v)).toBe(true);
    expect(numericValue(v)).toBe(47.4);
    expect(unitOf(v)).toBe('kJ/kg');
    expect(formatValue(v)).toBe(`${(47.4).toLocaleString()} kJ/kg`);
  });

  it.each(['INF', '-INF', 'NaN'])(
    'read a non-finite number (%s) as no number',
    (val) => {
      for (const v of [
        { _kind: 'number', val },
        { _kind: 'number', val, unit: 'm/s' },
      ]) {
        expect(isHaystackNumber(v)).toBe(true);
        expect(numericValue(v)).toBeNull();
        expect(formatValue(v)).toBe('—');
      }
    },
  );

  it('do not take another string val for a number', () => {
    expect(isHaystackNumber({ _kind: 'number', val: 'x' })).toBe(false);
  });
});
