import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type WithoutChildrenOrChild<T> = Omit<T, "children" | "child">;
export type WithoutChild<T> = Omit<T, "child">;
export type WithElementRef<T> = T & { ref?: HTMLElement | null };

/**
 * Haystack-encoded number, e.g. `{ _kind: "number", val: 47.4, unit: "kJ/kg" }`.
 * libhaystack serializes a unit-bearing `Number` via `serialize_map`, which
 * `serde-wasm-bindgen` lowers to a JS `Map`; plain JSON paths give an object.
 * Unit-less numbers cross the wasm boundary as primitives.
 */
export type HaystackNumberRecord = { _kind: "number"; val: number; unit?: string };

function readField(v: object, key: string): unknown {
	if (v instanceof Map) return v.get(key);
	return (v as Record<string, unknown>)[key];
}

/** True if `v` is a Haystack number, whether encoded as a JS Map or a plain object. */
export function isHaystackNumber(v: unknown): boolean {
	if (typeof v !== "object" || v === null) return false;
	return readField(v, "_kind") === "number" && typeof readField(v, "val") === "number";
}

/** Strip the Haystack number wrapper so consumers see a primitive number. */
export function numericValue(v: unknown): number | null {
	if (typeof v === "number") return v;
	if (isHaystackNumber(v)) return readField(v as object, "val") as number;
	if (typeof v === "string") {
		const n = Number(v);
		return Number.isFinite(n) ? n : null;
	}
	return null;
}

/** Pull the unit symbol from a unit-bearing Haystack number, if any. */
export function unitOf(v: unknown): string | undefined {
	if (!isHaystackNumber(v)) return undefined;
	const unit = readField(v as object, "unit");
	return typeof unit === "string" ? unit : undefined;
}

/** Format any block value (primitive, bool, Haystack number, array, object) for display. */
export function formatValue(v: unknown, opts?: { maxFractionDigits?: number }): string {
	if (v == null) return "—";
	const max = opts?.maxFractionDigits ?? 4;
	if (isHaystackNumber(v)) {
		const val = numericValue(v) ?? 0;
		const unit = unitOf(v);
		const num = Intl.NumberFormat(undefined, { maximumFractionDigits: max }).format(val);
		return unit ? `${num} ${unit}` : num;
	}
	if (typeof v === "number") {
		return Intl.NumberFormat(undefined, { maximumFractionDigits: max }).format(v);
	}
	if (typeof v === "boolean") return v ? "true" : "false";
	if (Array.isArray(v)) return "[]";
	if (v instanceof Map) return "{}";
	if (typeof v === "object") return "{}";
	return String(v);
}
