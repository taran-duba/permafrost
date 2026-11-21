import { useEffect, useRef, useState, type ComponentType } from "react";
import { AnimatePresence, motion, useInView, useScroll, useSpring } from "motion/react";
import { BrowseScene, CheckinScene, CheckoutScene, NamesScene } from "./scenes";
import { LiteScene } from "./LiteWindow";
import { useMediaQuery } from "../lib/phases";

type App = "permafrost" | "lite" | "both";
type Step = { tag: string; app: App; title: string; body: string; points: string[]; Scene: ComponentType<{ active: boolean }> };

const APP_LABEL: Record<App, string> = { permafrost: "Permafrost", lite: "Permafrost Lite", both: "Both apps" };

const STEPS: Step[] = [
  {
    tag: "Browse",
    app: "permafrost",
    title: "Opens instantly. Every time.",
    body: "Permafrost keeps a local index of your CAD folder (a whole shared drive, or one folder inside it) and updates it from Drive's change feed. Browsing and searching never wait on the network.",
    points: ["Local SQLite index", "Incremental change feed", "Pick which folders land on this PC"],
    Scene: BrowseScene,
  },
  {
    tag: "Check out",
    app: "permafrost",
    title: "Check a file out and it's yours.",
    body: "Checking out locks the file on Drive, so nobody overwrites your work. Anything you haven't checked out stays read-only on your PC, and locks left sitting for days get flagged so you can chase them down.",
    points: ["Drive content restriction plus lock properties", "Read-only until it's yours", "See who has what, on which PC"],
    Scene: CheckoutScene,
  },
  {
    tag: "Push",
    app: "permafrost",
    title: "Assemblies arrive whole.",
    body: "An assembly and its changed parts upload together, or not at all. Before anything leaves your PC, Permafrost asks SolidWorks for the real references and stops on missing files, files outside the vault and duplicate names.",
    points: ["Atomic pushes", "References read by SolidWorks itself", "A plain-language fix for every problem"],
    Scene: CheckinScene,
  },
  {
    tag: "Sync",
    app: "both",
    title: "Any name Drive allows, Windows accepts.",
    body: "Slashes, reserved names like CON, trailing dots and names that differ only by case all get safe local names. Drive keeps the originals, and every PC works out the same paths, so references line up for the whole team.",
    points: ["Deterministic path mapping", "Crash-safe sync journal", "Runs quietly in the background"],
    Scene: NamesScene,
  },
  {
    tag: "Lite",
    app: "lite",
    title: "Or skip check-outs entirely.",
    body: "Permafrost Lite works the way CacheCAD did: two lists, Upload Changes and Download Changes, each with a count. Tick what you want and apply it. Every file is tagged New, Revision, Delete or Mismatch, and a file a teammate changed since you last downloaded it is never overwritten unless you say so.",
    points: [
      "Downloads verified, then applied all at once",
      "Your edits backed up before anything replaces them",
      "Shares a Drive folder with Permafrost users",
    ],
    Scene: LiteScene,
  },
];

export function Story() {
  const wide = useMediaQuery("(min-width: 960px)");
  const [active, setActive] = useState(0);
  const section = useRef<HTMLElement>(null);
  const { scrollYProgress } = useScroll({ target: section, offset: ["start center", "end center"] });
  const fill = useSpring(scrollYProgress, { stiffness: 140, damping: 30, restDelta: 0.001 });

  return (
    <section id="how" className="story" ref={section} aria-labelledby="story-title">
      <div className="container">
        <header className="section-head">
          <p className="eyebrow">How it works</p>
          <h2 id="story-title" className="section-title">
            From your PC to the whole team, safely.
          </h2>
          <p className="section-lede">
            Permafrost's full workflow, step by step, then how Permafrost Lite keeps it to upload and download.
          </p>
        </header>

        <div className={wide ? "story-grid" : "story-stack"}>
          <div className="story-steps">
            {wide && (
              <div className="story-rail" aria-hidden>
                <motion.div className="story-rail-fill" style={{ scaleY: fill }} />
              </div>
            )}
            {STEPS.map((s, i) => (
              <StepText key={s.tag} step={s} index={i} active={active === i} onEnter={setActive} inline={!wide} />
            ))}
          </div>

          {wide && (
            <div className="story-stage">
              <div className="story-stage-inner">
                <div className="story-tabs" aria-hidden>
                  {STEPS.map((s, i) => (
                    <span key={s.tag} className={`story-tab ${i === active ? "on" : ""}`}>
                      {i === active && <motion.span layoutId="story-tab" className="story-tab-bg" />}
                      {s.tag}
                    </span>
                  ))}
                </div>
                <AnimatePresence mode="wait">
                  <motion.div
                    key={active}
                    initial={{ opacity: 0, y: 16, scale: 0.985 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.985, transition: { duration: 0.18 } }}
                    transition={{ type: "spring", bounce: 0, duration: 0.5 }}
                  >
                    {(() => {
                      const Scene = STEPS[active].Scene;
                      return <Scene active />;
                    })()}
                  </motion.div>
                </AnimatePresence>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function StepText({
  step,
  index,
  active,
  onEnter,
  inline,
}: {
  step: Step;
  index: number;
  active: boolean;
  onEnter: (i: number) => void;
  inline: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // A thin band across the middle of the viewport decides which step is current.
  const centered = useInView(ref, { margin: "-48% 0px -48% 0px" });
  const visible = useInView(ref, { amount: 0.35 });
  useEffect(() => {
    if (centered) onEnter(index);
  }, [centered, index, onEnter]);

  const lit = inline ? visible : active;
  const { Scene } = step;

  return (
    <motion.article
      ref={ref}
      className={`story-step ${lit ? "on" : ""}`}
      initial={{ opacity: 0, y: 30 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, amount: 0.3 }}
      transition={{ duration: 0.6, ease: [0.16, 1, 0.3, 1] }}
    >
      <span className={`app-tag ${step.app}`}>{APP_LABEL[step.app]}</span>
      <p className="story-num">
        <span className="story-idx mono">0{index + 1}</span> {step.tag}
      </p>
      <h3 className="story-title">{step.title}</h3>
      <p className="story-body">{step.body}</p>
      <ul className="story-points">
        {step.points.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      {inline && (
        <div className="story-inline">
          <Scene active={visible} />
        </div>
      )}
    </motion.article>
  );
}
