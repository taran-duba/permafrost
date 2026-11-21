import { useEffect } from "react";
import { motion } from "motion/react";
import { ArrowUpRight, Download as DownloadIcon } from "lucide-react";
import { AppleIcon, WindowsIcon } from "./BrandIcons";
import { hrefFor } from "./DownloadButtons";
import {
  formatSize,
  MAC_BUILD_URL,
  noInstallers,
  RELEASES_URL,
  SETUP_URL,
  useRelease,
  type Product,
} from "../lib/release";
import { setProduct, useProduct } from "../lib/product";

const ease = [0.16, 1, 0.3, 1] as const;

/** Links to these (e.g. from the Products section) open the Download section on that app. */
export const DOWNLOAD_HASH: Record<Product, string> = {
  permafrost: "#download-permafrost",
  lite: "#download-lite",
};

const NOTES: Record<Product, { windows: string[]; mac: string }> = {
  permafrost: {
    windows: [
      "Reference checking needs SolidWorks installed on the same PC.",
      "Installs per user. No admin rights needed for the .exe.",
    ],
    mac: "SolidWorks doesn't run on macOS, so reference checking is off. Syncing, locks and pushing changes all work.",
  },
  lite: {
    windows: [
      "Lite moves files as they are and doesn't check SolidWorks references.",
      "Installs per user. No admin rights needed for the .exe.",
    ],
    mac: "Upload and download changes work the same as on Windows.",
  },
};

export function Download() {
  const product = useProduct();
  // Fetch both up front so flipping the toggle never waits on the network.
  const releases = { permafrost: useRelease("permafrost"), lite: useRelease("lite") };
  const release = releases[product];
  const info = release.status === "ready" ? release.info : null;
  const comingSoon = noInstallers(release);
  const macMissing = !!info && !info.mac && !comingSoon;

  useEffect(() => {
    const sync = () => {
      if (window.location.hash === DOWNLOAD_HASH.lite) setProduct("lite");
      if (window.location.hash === DOWNLOAD_HASH.permafrost) setProduct("permafrost");
    };
    sync();
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);

  const soon = (
    <a className="btn btn-lg dl-main" href={RELEASES_URL}>
      <ArrowUpRight size={18} />
      <span className="btn-stack">
        Coming soon
        <span className="btn-sub">Follow GitHub Releases</span>
      </span>
    </a>
  );

  return (
    <section id="download" className="download" aria-labelledby="download-title">
      <span id={DOWNLOAD_HASH.permafrost.slice(1)} />
      <span id={DOWNLOAD_HASH.lite.slice(1)} />
      <div className="download-aurora" aria-hidden />
      <div className="container">
        <header className="section-head center">
          <p className="eyebrow">Download</p>
          <h2 id="download-title" className="section-title">
            Freeze your CAD in place.
          </h2>
          <p className="section-lede">
            Install it on every PC that opens the robot. One teammate does the{" "}
            <a className="text-link" href={SETUP_URL}>
              one-time team setup
            </a>{" "}
            first, then everyone else just signs in.
          </p>
        </header>

        <div className="product-toggle" role="group" aria-label="Which app to download">
          {(["permafrost", "lite"] as const).map((p) => (
            <button
              key={p}
              type="button"
              className={product === p ? "active" : ""}
              aria-pressed={product === p}
              onClick={() => setProduct(p)}
            >
              {p === "lite" ? "Lite" : "Permafrost"}
            </button>
          ))}
        </div>

        <div className="dl-grid" aria-live="polite">
          <motion.article
            className="dl-card featured"
            initial={{ opacity: 0, y: 30 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, amount: 0.3 }}
            transition={{ duration: 0.6, ease }}
          >
            <div className="dl-os">
              <WindowsIcon size={26} />
              <div>
                <h3>Windows</h3>
                <p>Windows 10 &amp; 11, 64-bit</p>
              </div>
            </div>
            {comingSoon ? (
              soon
            ) : (
              <>
                <a className="btn btn-primary btn-lg dl-main" href={hrefFor(release, "windows", product)}>
                  <DownloadIcon size={18} />
                  <span className="btn-stack">
                    {product === "lite" ? "Download Permafrost Lite" : "Download installer"}
                    <span className="btn-sub">
                      {info?.windows ? `v${info.windows.version} · .exe · ${formatSize(info.windows.size)}` : ".exe"}
                    </span>
                  </span>
                </a>
                <a className="dl-alt" href={hrefFor(release, "windowsMsi", product)}>
                  Prefer an MSI for school-managed PCs? <span>Get the .msi</span>
                </a>
              </>
            )}
            <ul className="dl-notes">
              {NOTES[product].windows.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </motion.article>

          <motion.article
            className="dl-card"
            initial={{ opacity: 0, y: 30 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, amount: 0.3 }}
            transition={{ duration: 0.6, ease, delay: 0.08 }}
          >
            <div className="dl-os">
              <AppleIcon size={26} />
              <div>
                <h3>macOS</h3>
                <p>Apple Silicon (M1 and newer)</p>
              </div>
            </div>
            {comingSoon ? (
              soon
            ) : macMissing ? (
              <a className="btn btn-lg dl-main" href={product === "lite" ? RELEASES_URL : MAC_BUILD_URL}>
                <ArrowUpRight size={18} />
                <span className="btn-stack">
                  {product === "lite" ? "Mac build coming soon" : "Build from source"}
                  <span className="btn-sub">No packaged Mac release yet</span>
                </span>
              </a>
            ) : (
              <a className="btn btn-lg dl-main" href={hrefFor(release, "mac", product)}>
                <DownloadIcon size={18} />
                <span className="btn-stack">
                  {product === "lite" ? "Download Lite for Mac" : "Download for Mac"}
                  <span className="btn-sub">
                    {info?.mac ? `v${info.mac.version} · .dmg · ${formatSize(info.mac.size)}` : ".dmg"}
                  </span>
                </span>
              </a>
            )}
            <ul className="dl-notes">
              {!macMissing && !comingSoon && (
                <li>Not notarized yet: the first time, right-click the app and choose Open.</li>
              )}
              <li>{NOTES[product].mac}</li>
            </ul>
          </motion.article>
        </div>

        <p className="dl-foot">
          Not sure which one?{" "}
          <a className="text-link" href="#products">
            Compare Permafrost and Lite
          </a>
          <span aria-hidden> · </span>
          <a className="text-link" href={RELEASES_URL}>
            All releases on GitHub <ArrowUpRight size={14} />
          </a>
        </p>
      </div>
    </section>
  );
}
