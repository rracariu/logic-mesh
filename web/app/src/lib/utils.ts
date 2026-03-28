import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type WithoutChildrenOrChild<T> = Omit<T, "children" | "child">;
export type WithoutChild<T> = Omit<T, "child">;
export type WithElementRef<T> = T & { ref?: HTMLElement | null };
