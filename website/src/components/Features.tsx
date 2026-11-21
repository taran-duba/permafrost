import { motion } from "motion/react";
import { Archive, ArrowUpDown, FileLock2, LifeBuoy, ScanSearch, ShieldCheck, Zap } from "lucide-react";
import type { LucideIcon } from "lucide-react";

type App = "permafrost" | "lite" | "both";
type Feature = { icon: LucideIcon; title: string; body: string; app: App; wide?: boolean; tone?: string };

const APP_LABEL: Record<App, string> = { permafrost: "Permafrost", lite: "Permafrost Lite", both: "Both apps" };

const FEATURES: Feature[] = [
  {
    icon: Zap,
    title: "Built for slow school Wi-Fi",
    body: "The index lives on your PC, so lists appear instantly and Drive is checked in the background. Only changed files move.",
    app: "both",
    wide: true,
  },
  {
    icon: ShieldCheck,
    title: "No more crashes on weird names",
    body: "Emoji, slashes, CON and duplicate names all map to safe Windows names, and a crash-safe journal finishes what it started.",
    app: "both",
  },
  {
    icon: FileLock2,
    title: "Nobody overwrites anybody",
    body: "Check-out locks are stored on Drive itself, and files you haven't checked out are read-only on disk.",
    app: "permafrost",
  },
  {
    icon: ScanSearch,
    title: "Mates that don't break",
    body: "SolidWorks itself reports each assembly's references, and pushes stop on missing files before teammates ever see them.",
    app: "permafrost",
  },
  {
    icon: LifeBuoy,
    title: "Your work is never thrown away",
    body: "If an update lands on a file you edited, your edits are kept: as a separate copy in Permafrost, in a backups folder in Lite.",
    app: "both",
  },
  {
    icon: Archive,
    title: "A clear history of pushes",
    body: "Every push is logged with who made it, when, how many files went up, and the message they wrote. Lite uploads show up there too.",
    app: "permafrost",
  },
  {
    icon: ArrowUpDown,
    title: "Or keep it as simple as CacheCAD",
    body: "Permafrost Lite gives you two counts, Upload Changes and Download Changes, and a list to tick. No check-outs to learn, and a warning with an explicit Overwrite whenever someone else changed the same file.",
    app: "lite",
    wide: true,
  },
];

export function Features() {
  return (
    <section id="features" className="features" aria-labelledby="features-title">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">Why teams switch</p>
          <h2 id="features-title" className="section-title">
            Every way shared CAD used to melt down, fixed.
          </h2>
        </header>
        <div className="bento">
          {FEATURES.map((f, i) => (
            <motion.article
              key={f.title}
              className={`card ${f.wide ? "wide" : ""}`}
              initial={{ opacity: 0, y: 28 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true, amount: 0.25 }}
              transition={{ duration: 0.6, ease: [0.16, 1, 0.3, 1], delay: (i % 3) * 0.06 }}
            >
              <span className="card-icon">
                <f.icon size={28} strokeWidth={1.5} />
              </span>
              <span className={`app-tag ${f.app}`}>{APP_LABEL[f.app]}</span>
              <h3>{f.title}</h3>
              <p>{f.body}</p>
            </motion.article>
          ))}
        </div>
      </div>
    </section>
  );
}
