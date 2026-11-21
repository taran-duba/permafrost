import { useSyncExternalStore } from "react";
import type { Product } from "./release";

/** Which app the visitor is looking at. The hero switch sets it, and the Download section follows. */
let current: Product = "permafrost";
const listeners = new Set<() => void>();

export function setProduct(p: Product) {
  if (p === current) return;
  current = p;
  listeners.forEach((l) => l());
}

export function useProduct(): Product {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
    () => "permafrost" as const, // prerendered HTML always starts on Permafrost
  );
}

export const PRODUCT_NAME: Record<Product, string> = { permafrost: "Permafrost", lite: "Permafrost Lite" };
