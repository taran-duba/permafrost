import { motion } from "motion/react";
import { ArrowRight, Check, Minus } from "lucide-react";
import { DOWNLOAD_HASH } from "./Download";

const ease = [0.16, 1, 0.3, 1] as const;

type Row = { label: string; full: boolean; lite: boolean; note?: string };

const ROWS: Row[] = [
  { label: "Check-out locks", full: true, lite: false, note: "Conflict warning instead" },
  { label: "SolidWorks reference checks before pushing", full: true, lite: false },
  { label: "Rename and move with reference rewriting", full: true, lite: false },
  { label: "History and restore", full: true, lite: false },
  { label: "Works alongside the other app on the same Drive folder", full: true, lite: true },
];

function Mark({ yes, note }: { yes: boolean; note?: string }) {
  return (
    <span className={`cmp-mark ${yes ? "yes" : "no"}`}>
      {yes ? <Check size={18} aria-hidden /> : <Minus size={18} aria-hidden />}
      <span className="sr-only">{yes ? "Yes" : "No"}</span>
      {note && <span className="cmp-note">{note}</span>}
    </span>
  );
}

const reveal = (delay = 0) => ({
  initial: { opacity: 0, y: 28 },
  whileInView: { opacity: 1, y: 0 },
  viewport: { once: true, amount: 0.25 },
  transition: { duration: 0.6, ease, delay },
});

export function Products() {
  return (
    <section id="products" className="products" aria-labelledby="products-title">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">Two apps, one Drive folder</p>
          <h2 id="products-title" className="section-title">
            Pick the workflow your team already knows.
          </h2>
          <p className="section-lede">
            Both sync SolidWorks files through the same Google Drive folder, so teammates can use whichever fits them.
          </p>
        </header>

        <div className="product-grid">
          <motion.article className="product-card featured" {...reveal()}>
            <p className="product-kicker mono">The full workflow</p>
            <h3>Permafrost</h3>
            <p className="product-lede">Check out what you're editing, push assemblies and parts together, and never break a mate.</p>
            <ul className="product-list">
              <li>Check-out locks stored on Drive, so nobody overwrites anybody</li>
              <li>Atomic pushes: an assembly and its changed parts land together, after SolidWorks reference checks</li>
              <li>Rename and move files with every reference rewritten</li>
              <li>History of every push, with restore</li>
              <li>Team roles for who can do what</li>
            </ul>
            <a className="btn btn-primary product-cta" href={DOWNLOAD_HASH.permafrost}>
              Get Permafrost <ArrowRight size={16} />
            </a>
          </motion.article>

          <motion.article className="product-card" {...reveal(0.08)}>
            <p className="product-kicker mono">Simple and familiar</p>
            <h3>Permafrost Lite</h3>
            <p className="product-lede">
              Two panels, Upload Changes and Download Changes, each with a count. Tick what you want and apply.
            </p>
            <ul className="product-list">
              <li>
                Every item is tagged{" "}
                <span className="product-tags">
                  <span className="tag new">New</span>
                  <span className="tag rev">Revision</span>
                  <span className="tag del">Delete</span>
                  <span className="tag mis">Mismatch</span>
                </span>
              </li>
              <li>No locks: if someone else changed a file, you get a warning and an explicit Overwrite option</li>
              <li>Feels like home for teams coming from CacheCAD</li>
            </ul>
            <a className="btn product-cta" href={DOWNLOAD_HASH.lite}>
              Get Permafrost Lite <ArrowRight size={16} />
            </a>
          </motion.article>
        </div>

        <motion.div className="cmp-wrap" {...reveal()}>
          <table className="cmp">
            <caption className="sr-only">Permafrost and Permafrost Lite compared</caption>
            <thead>
              <tr>
                <th scope="col">
                  <span className="sr-only">Feature</span>
                </th>
                <th scope="col">Permafrost</th>
                <th scope="col">Lite</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((r) => (
                <tr key={r.label}>
                  <th scope="row">{r.label}</th>
                  <td>
                    <Mark yes={r.full} />
                  </td>
                  <td>
                    <Mark yes={r.lite} note={r.note} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </motion.div>

        <p className="product-pick">
          <strong>Which should I pick?</strong> Coming from CacheCAD, or just want upload and download? Start with Lite.
          Want locks and assemblies that never break? Get Permafrost.
        </p>
      </div>
    </section>
  );
}
