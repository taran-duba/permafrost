import { useSyncExternalStore, type ReactNode } from "react";
import { detectPlatform } from "../lib/platform";
import { MAC_BUILD_URL, noInstallers, RELEASES_URL, useRelease, type Product, type ReleaseState } from "../lib/release";
import { AppleIcon, WindowsIcon } from "./BrandIcons";

type Choice = { id: "windows" | "mac"; label: string; href: string; icon: ReactNode; available: boolean };

/** Direct installer URL once known; /download/[lite/]<platform> (which redirects) while loading. */
export function hrefFor(
  state: ReleaseState,
  id: "windows" | "mac" | "windowsMsi",
  product: Product = "permafrost",
): string {
  if (state.status === "error") return RELEASES_URL;
  if (state.status === "loading")
    return `/download/${product === "lite" ? "lite/" : ""}${id === "windowsMsi" ? "windows-msi" : id}`;
  return state.info[id]?.url ?? RELEASES_URL;
}

const noSubscribe = () => () => {};

/** Hero call to action for the chosen app: the visitor's own OS first, the other one beside it. */
export function DownloadButtons({ product = "permafrost" }: { product?: Product }) {
  const release = useRelease(product);
  // "other" while prerendering, so the static HTML and the first client render agree.
  const platform = useSyncExternalStore(noSubscribe, detectPlatform, () => "other" as const);
  const soon = noInstallers(release);
  const macMissing = release.status === "ready" && !release.info.mac && !soon;
  const version =
    release.status === "ready" ? (release.info.windows?.version ?? release.info.mac?.version) : undefined;
  const name = product === "lite" ? "Permafrost Lite" : "Permafrost";

  const win: Choice = {
    id: "windows",
    label: "Download for Windows",
    href: hrefFor(release, "windows", product),
    icon: <WindowsIcon size={17} />,
    available: true,
  };
  const mac: Choice = {
    id: "mac",
    label: "Download for macOS",
    href: hrefFor(release, "mac", product),
    icon: <AppleIcon size={18} />,
    available: !macMissing,
  };
  const [first, second] = platform === "mac" && mac.available ? [mac, win] : [win, mac];

  return (
    <div className="cta">
      <div className="cta-row">
        {soon ? (
          <a className="btn btn-primary btn-lg" href={RELEASES_URL}>
            {name}: coming soon
          </a>
        ) : (
          <>
            <a className="btn btn-primary btn-lg" href={first.href} aria-label={`${first.label}: ${name}`}>
              {first.icon}
              {first.label}
            </a>
            {second.available ? (
              <a className="btn btn-lg" href={second.href} aria-label={`${name} for ${second.id === "mac" ? "macOS" : "Windows"}`}>
                {second.icon}
                {second.id === "mac" ? "macOS" : "Windows"}
              </a>
            ) : (
              <a className="btn btn-lg btn-ghost" href={MAC_BUILD_URL}>
                <AppleIcon size={18} />
                macOS: build from source
              </a>
            )}
          </>
        )}
      </div>
      <p className="cta-meta">
        {soon ? (
          <span>Follow GitHub Releases for the first build</span>
        ) : version ? (
          <span className="mono">
            {name} v{version}
          </span>
        ) : (
          <span className="cta-meta-skel" />
        )}
        <span aria-hidden>·</span>
        <span>Windows 10 &amp; 11 (x64){macMissing ? "" : " and Apple Silicon Macs"}</span>
        <span aria-hidden>·</span>
        <span>Free and open source</span>
      </p>
    </div>
  );
}
