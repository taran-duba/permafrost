import { useRef } from "react";
import { AnimatePresence, motion, useScroll, useTransform, useReducedMotion } from "motion/react";
import { ArrowDown, ArrowRight } from "lucide-react";
import { HeroScene } from "./scenes";
import { LiteScene } from "./LiteWindow";
import { DownloadButtons } from "./DownloadButtons";
import { setProduct, useProduct } from "../lib/product";
import type { Product } from "../lib/release";

const ease = [0.16, 1, 0.3, 1] as const;
const rise = (delay: number) => ({
  initial: { opacity: 0, y: 14 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: 0.8, ease, delay },
});

const COPY: Record<Product, { lede: string; scroll: string }> = {
  permafrost: {
    lede: "Permafrost keeps your team's SolidWorks files in sync through Google Drive. Check a file out, edit it, push it back. No overwrites, no broken mates, no staring at a spinner while the network catches up.",
    scroll: "See how Permafrost works",
  },
  lite: {
    lede: "Permafrost Lite is CacheCAD's workflow without the crashes: one list of changes to upload, one to download. Tick what you want and apply. No check-outs to learn, and nothing changed by a teammate gets overwritten without asking.",
    scroll: "See how Lite works",
  },
};

const OPTIONS: { id: Product; name: string; sub: string }[] = [
  { id: "permafrost", name: "Permafrost", sub: "Check-out locks & atomic pushes" },
  { id: "lite", name: "Permafrost Lite", sub: "Simple upload & download" },
];

export function Hero() {
  const product = useProduct();
  const stage = useRef<HTMLDivElement>(null);
  const reduce = useReducedMotion();
  // The windows start tilted back like a slab of ice and settle flat as you scroll to them.
  const { scrollYProgress } = useScroll({ target: stage, offset: ["start end", "start 0.25"] });
  const rotateX = useTransform(scrollYProgress, [0, 1], reduce ? [0, 0] : [24, 0]);
  const scale = useTransform(scrollYProgress, [0, 1], reduce ? [1, 1] : [0.9, 1]);
  const y = useTransform(scrollYProgress, [0, 1], reduce ? [0, 0] : [40, 0]);
  const glow = useTransform(scrollYProgress, [0, 1], [0.35, 1]);
  const lite = product === "lite";

  return (
    <section className="hero" aria-labelledby="hero-title">
      <div className="hero-frost" aria-hidden />
      <div className="container hero-copy">
        <motion.button type="button" className="hero-news" onClick={() => setProduct("lite")} {...rise(0)}>
          <span className="hero-news-tag">New</span>
          Permafrost Lite: CacheCAD's workflow, without the crashes
          <ArrowRight size={15} />
        </motion.button>
        <motion.h1 id="hero-title" className="hero-title" {...rise(0.08)}>
          Shared CAD that <span className="accent-text">stays solid</span>.
        </motion.h1>

        <motion.div className="product-switch" role="tablist" aria-label="Choose an app" {...rise(0.14)}>
          {OPTIONS.map((o) => (
            <button
              key={o.id}
              type="button"
              role="tab"
              aria-selected={product === o.id}
              aria-controls="hero-panel"
              onClick={() => setProduct(o.id)}
            >
              {product === o.id && <motion.span layoutId="product-switch-bg" className="ps-bg" transition={{ type: "spring", bounce: 0.15, duration: 0.45 }} />}
              <span className="ps-name">{o.name}</span>
              <span className="ps-sub">{o.sub}</span>
            </button>
          ))}
        </motion.div>

        <div id="hero-panel" role="tabpanel" aria-live="polite">
          <AnimatePresence mode="wait" initial={false}>
            <motion.p
              key={product}
              className="hero-lede"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6, transition: { duration: 0.15 } }}
              transition={{ duration: 0.35, ease }}
            >
              {COPY[product].lede}
            </motion.p>
          </AnimatePresence>
          <motion.div {...rise(0.24)}>
            <DownloadButtons product={product} />
          </motion.div>
        </div>
        <motion.a href="#how" className="hero-scroll" {...rise(0.4)}>
          {COPY[product].scroll} <ArrowDown size={15} />
        </motion.a>
      </div>

      <div className="container hero-stage" ref={stage}>
        <motion.div className="hero-glow" style={{ opacity: glow }} aria-hidden />
        <motion.div
          className="hero-window"
          style={{ rotateX, scale, y, transformPerspective: 1400 }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 1, ease, delay: 0.3 }}
        >
          <div className="hero-pair">
            <div className={`hero-back ${lite ? "dim" : ""}`} onClick={() => setProduct("permafrost")}>
              <span className={`hero-tag back ${lite ? "" : "on"}`}>Permafrost</span>
              <HeroScene />
            </div>
            <div className={`hero-front ${lite ? "" : "dim"}`} onClick={() => setProduct("lite")}>
              <span className={`hero-tag front ${lite ? "on" : ""}`}>Permafrost Lite</span>
              <LiteScene />
            </div>
          </div>
        </motion.div>
      </div>
    </section>
  );
}
