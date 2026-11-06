import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, 'child'> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any }
  ? Omit<T, 'children'>
  : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
  ref?: U | null;
};

/**
 * Haystack-encoded number, e.g. `{ _kind: "number", val: 47.4, unit: "kJ/kg" }`.
 * The engine hands a unit-bearing `Number` to JS as a plain object; a JS
 * `Map` of the same entries is read too. Unit-less numbers cross the wasm
 * boundary as primitives, except the non-finite ones: Hayson has no JSON
 * number for them, so their `val` is `"INF"`, `"-INF"` or `"NaN"`.
 */
export type HaystackNumberRecord = {
  _kind: 'number';
  val: number | NonFiniteVal;
  unit?: string;
};

/** The Hayson `val` of a non-finite number. */
type NonFiniteVal = 'INF' | '-INF' | 'NaN';

const NON_FINITE_VALS: ReadonlySet<unknown> = new Set(['INF', '-INF', 'NaN']);

function readField(v: object, key: string): unknown {
  if (v instanceof Map) return v.get(key);
  return (v as Record<string, unknown>)[key];
}

/** True if `v` is a Haystack number, whether encoded as a JS Map or a plain object. */
export function isHaystackNumber(v: unknown): boolean {
  if (typeof v !== 'object' || v === null) return false;
  return (
    readField(v, '_kind') === 'number' &&
    (typeof readField(v, 'val') === 'number' ||
      NON_FINITE_VALS.has(readField(v, 'val')))
  );
}

/**
 * Strip the Haystack number wrapper so consumers see a primitive number.
 * A non-finite number reads as `null`, no number, as it does for the
 * widgets and readouts.
 */
export function numericValue(v: unknown): number | null {
  if (typeof v === 'number') return v;
  if (isHaystackNumber(v)) {
    const val = readField(v as object, 'val');
    return typeof val === 'number' ? val : null;
  }
  if (typeof v === 'string') {
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

/** Pull the unit symbol from a unit-bearing Haystack number, if any. */
export function unitOf(v: unknown): string | undefined {
  if (!isHaystackNumber(v)) return undefined;
  const unit = readField(v as object, 'unit');
  return typeof unit === 'string' ? unit : undefined;
}

/** Format any block value (primitive, bool, Haystack number, array, object) for display. */
export function formatValue(
  v: unknown,
  opts?: { maxFractionDigits?: number },
): string {
  if (v == null) return '—';
  const max = opts?.maxFractionDigits ?? 4;
  if (isHaystackNumber(v)) {
    const val = numericValue(v);
    if (val == null) return '—';
    const unit = unitOf(v);
    const num = Intl.NumberFormat(undefined, {
      maximumFractionDigits: max,
    }).format(val);
    return unit ? `${num} ${unit}` : num;
  }
  if (typeof v === 'number') {
    return Intl.NumberFormat(undefined, { maximumFractionDigits: max }).format(
      v,
    );
  }
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (Array.isArray(v)) return '[]';
  if (v instanceof Map) return '{}';
  if (typeof v === 'object') return '{}';
  return String(v);
}
