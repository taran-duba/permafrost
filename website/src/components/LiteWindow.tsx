import { AnimatePresence, motion } from "motion/react";
import { usePhases } from "../lib/phases";
import { CheckBox, FileName, Spinner } from "./mockParts";

type Kind = "new" | "rev" | "del" | "mis";
const KIND_GLYPH: Record<Kind, string> = { new: "+", rev: "↻", del: "−", mis: "!" };

/** The Permafrost Lite window, rebuilt in HTML: CacheCAD's sidebar plus the two change panels. */
function LiteWindow({ label, children, loaded = "2026 Robot" }: { label: string; children: React.ReactNode; loaded?: string }) {
  return (
    <div className="mock mock-lite" role="img" aria-label={label}>
      <div aria-hidden>
        <div className="mock-chrome">
          <span className="mock-dot" />
          <span className="mock-dot" />
          <span className="mock-dot" />
          <span className="mock-title">Permafrost Lite</span>
        </div>
        <div className="lite-mock-body">
          <div className="lite-mock-side">
            <div className="lite-mock-row">
              <span className="lite-key">Sign out</span>
              <span className="lite-key">Settings</span>
            </div>
            <span className="lite-key">▭ Permafrost Lite</span>
            <div className="lite-mock-projects">
              {["2026 Robot", "Off-season Swerve", "Shop Fixtures"].map((p) => (
                <span key={p} className={`lite-mock-project ${p === loaded ? "on" : ""}`}>
                  {p}
                </span>
              ))}
            </div>
            <div className="lite-mock-row">
              <span className="lite-key">New Project</span>
              <span className="lite-key">Remove</span>
            </div>
          </div>
          <div className="lite-mock-main">
            <div className="lite-mock-title">{loaded}</div>
            {children}
          </div>
        </div>
      </div>
    </div>
  );
}

function Panels({ up, down, pressed }: { up: number; down: number; pressed?: "down" }) {
  return (
    <div className="lite-mock-panels">
      <div className="lite-mock-panel">
        <span className="lite-key">Upload Changes</span>
        <motion.span key={`u${up}`} className="lite-mock-count" initial={{ opacity: 0.4, y: -4 }} animate={{ opacity: 1, y: 0 }}>
          {up}
        </motion.span>
        <span className="lite-mock-caption">Files ready to upload</span>
        <span className="lite-key">Open Local Folder</span>
      </div>
      <div className="lite-mock-panel">
        <span className={`lite-key ${pressed === "down" ? "pressed" : ""}`}>Download Changes</span>
        <motion.span key={`d${down}`} className="lite-mock-count" initial={{ opacity: 0.4, y: -4 }} animate={{ opacity: 1, y: 0 }}>
          {down}
        </motion.span>
        <span className="lite-mock-caption">Files ready to download</span>
        <span className="lite-key">Open Google Drive</span>
      </div>
    </div>
  );
}

const ROWS: [Kind, string, string?][] = [
  ["rev", "Robot.SLDASM", "Jordan Lee · 2 min ago"],
  ["new", "Arm/Wrist Plate.SLDPRT", "Sam Ortiz · 5 min ago"],
  ["rev", "Drivetrain/Belly Pan.SLDPRT", "Jordan Lee · 9 min ago"],
  ["mis", "Arm/Pivot.SLDPRT", "Changed here and on Drive"],
];

function List({ busy }: { busy: boolean }) {
  return (
    <div className="lite-mock-list">
      <div className="lite-mock-listhead">
        <span className="lite-key">← Back</span>
        <b>Download Changes</b>
      </div>
      {ROWS.map(([kind, name, sub], i) => (
        <motion.div
          key={name}
          className="lite-mock-item"
          initial={{ opacity: 0, x: 8 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: i * 0.06 }}
        >
          <CheckBox on={kind !== "mis"} />
          <span className={`lite-kind ${kind}`}>{KIND_GLYPH[kind]}</span>
          <span className="lite-mock-name">
            <FileName name={name.split("/").pop() ?? name} />
            <span className={`lite-mock-sub ${kind === "mis" ? "warn" : ""}`}>{sub}</span>
          </span>
        </motion.div>
      ))}
      <div className="lite-mock-foot">
        <span className="lite-key lite-key-accent">
          {busy ? (
            <>
              <Spinner /> Downloading…
            </>
          ) : (
            "Download 3 items"
          )}
        </span>
      </div>
    </div>
  );
}

// Panels → press Download Changes → the list → downloading → panels again, now caught up.
const LITE_T = [2400, 500, 2600, 1300, 2800];

/** The animated Lite window used in the hero and in "How it works". */
export function LiteScene({ active = true }: { active?: boolean }) {
  const p = usePhases(LITE_T, active);
  const showList = p === 2 || p === 3;
  return (
    <LiteWindow label="Permafrost Lite: Upload Changes and Download Changes panels with counts, and a list of files to tick and download.">
      <AnimatePresence mode="wait" initial={false}>
        {showList ? (
          <motion.div key="list" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.2 }}>
            <List busy={p === 3} />
          </motion.div>
        ) : (
          <motion.div key="panels" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.2 }}>
            <Panels up={2} down={p >= 4 ? 1 : 4} pressed={p === 1 ? "down" : undefined} />
          </motion.div>
        )}
      </AnimatePresence>
    </LiteWindow>
  );
}
