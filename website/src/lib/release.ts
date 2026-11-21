import { useEffect, useState } from "react";
import type { Product, ReleaseInfo } from "../../api/release";

export type { Product };

export const REPO_URL = "https://github.com/taran-duba/permafrost";
export const RELEASES_URL = `${REPO_URL}/releases`;
export const SETUP_URL = `${REPO_URL}/blob/main/docs/SETUP.md`;
export const MAC_BUILD_URL = `${REPO_URL}#mac-build`;

export type ReleaseState =
  | { status: "loading" }
  | { status: "ready"; info: ReleaseInfo }
  | { status: "error" };

const cached: Partial<Record<Product, Promise<ReleaseInfo>>> = {};

function load(product: Product): Promise<ReleaseInfo> {
  const url = product === "lite" ? "/api/release?product=lite" : "/api/release";
  cached[product] ??= fetch(url)
    .then((r) => (r.ok ? (r.json() as Promise<ReleaseInfo>) : Promise.reject(new Error(String(r.status)))))
    .catch((e) => {
      delete cached[product]; // let a later mount retry
      throw e;
    });
  return cached[product];
}

/** Latest installers for one app from GitHub Releases (via /api/release, cached at the edge). */
export function useRelease(product: Product = "permafrost"): ReleaseState {
  const [state, setState] = useState<ReleaseState>({ status: "loading" });
  useEffect(() => {
    let live = true;
    setState({ status: "loading" });
    load(product).then(
      (info) => live && setState({ status: "ready", info }),
      () => live && setState({ status: "error" }),
    );
    return () => {
      live = false;
    };
  }, [product]);
  return state;
}

/** True once we know a product has no packaged installers at all (e.g. before Lite's first release). */
export function noInstallers(state: ReleaseState): boolean {
  return state.status === "ready" && !state.info.windows && !state.info.windowsMsi && !state.info.mac;
}

export function formatSize(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
}
