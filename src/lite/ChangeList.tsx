import { useEffect, useMemo, useRef, useState } from "react";
import { ChangeItem, Kind } from "./api";
import { who } from "../api";
import { Spinner } from "../Spinner";

const KIND_LABEL: Record<Kind, string> = {
  new: "New item",
  revision: "Revision",
  delete: "Delete",
  mismatch: "Mismatch",
  move: "Moved",
};

/** The icons from CacheCAD's quick reference, plus Moved. */
export function KindIcon({ kind }: { kind: Kind }) {
  const common = {
    className: `kind-icon kind-${kind}`,
    viewBox: "0 0 20 20",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    role: "img",
    "aria-label": KIND_LABEL[kind],
  };
  switch (kind) {
    case "new":
      return (
        <svg {...common}>
          <circle cx="10" cy="10" r="7.5" />
          <path d="M10 6.5v7M6.5 10h7" />
        </svg>
      );
    case "revision":
      return (
        <svg {...common}>
          <path d="M15.5 8.5A6 6 0 0 0 4.8 6.5M4.5 11.5a6 6 0 0 0 10.7 2" />
          <path d="M4.5 3.5v3h3M15.5 16.5v-3h-3" />
        </svg>
      );
    case "delete":
      return (
        <svg {...common}>
          <path d="M4 6h12M8 6V4.5h4V6M5.5 6l.8 10h7.4l.8-10" />
        </svg>
      );
    case "mismatch":
      return (
        <svg {...common}>
          <path d="M10 3 2.5 16.5h15z" />
          <path d="M10 8.5v3.5M10 14.4v.1" />
        </svg>
      );
    case "move":
      return (
        <svg {...common}>
          <path d="M3.5 10h12M11.5 6l4 4-4 4" />
        </svg>
      );
  }
}

export function Legend() {
  return (
    <div className="lite-legend" aria-label="Icon key">
      {(Object.keys(KIND_LABEL) as Kind[]).map((k) => (
        <span key={k}>
          <KindIcon kind={k} /> {KIND_LABEL[k]}
        </span>
      ))}
    </div>
  );
}

function when(iso: string | null) {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}

/** The line under a row's path: why it's a mismatch, why it's blocked, who changed it. */
function subLine(item: ChangeItem, direction: "upload" | "download") {
  if (item.blocked) return <span className="lite-sub bad">{item.blocked}. It can't be uploaded until it's checked back in.</span>;
  if (item.kind === "move" && item.fromPath) return <span className="lite-sub">Moved on Drive from {item.fromPath}</span>;
  if (item.kind === "mismatch") {
    const by = item.modifiedBy ? ` (last changed on Drive by ${who(item.modifiedBy)})` : "";
    const effect =
      direction === "upload"
        ? "Uploading replaces the Drive copy; theirs stays in Drive's version history."
        : "Downloading replaces your copy; yours is saved to the backups folder first.";
    return (
      <span className="lite-sub warn">
        {item.detail}
        {by}. {effect}
      </span>
    );
  }
  if (direction === "download" && item.modifiedBy && item.kind !== "delete")
    return (
      <span className="lite-sub">
        {who(item.modifiedBy)} · {when(item.modifiedTime)}
      </span>
    );
  return null;
}

export function ChangeList({
  direction,
  items,
  busy,
  onBack,
  onApply,
}: {
  direction: "upload" | "download";
  items: ChangeItem[];
  busy: boolean;
  onBack: () => void;
  onApply: (keys: string[], overwrite: string[], message: string) => void;
}) {
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [message, setMessage] = useState("");
  const title = direction === "upload" ? "Upload Changes" : "Download Changes";

  // Default ticks: everything except mismatches and blocked rows. Keep the user's
  // choices for rows that are still listed when the list refreshes.
  const keys = useMemo(() => items.map((i) => i.key).join("\n"), [items]);
  const seen = useRef<Set<string>>(new Set());
  useEffect(() => {
    setPicked((prev) => {
      const next = new Set<string>();
      for (const i of items) {
        if (i.blocked) continue;
        if (seen.current.has(i.key) ? prev.has(i.key) : i.checked) next.add(i.key);
      }
      return next;
    });
    seen.current = new Set(items.map((i) => i.key));
    // Only when the set of rows changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [keys]);

  const toggle = (item: ChangeItem, on: boolean) => {
    if (on && item.kind === "mismatch") {
      const verb = direction === "upload" ? "Upload your copy over the Drive copy" : "Download the Drive copy over yours";
      if (!window.confirm(`${item.path}\n\n${item.detail ?? "Changed on both sides"}.\n\n${verb}?`)) return;
    }
    setPicked((p) => {
      const n = new Set(p);
      if (on) n.add(item.key);
      else n.delete(item.key);
      return n;
    });
  };

  const selectable = items.filter((i) => !i.blocked);
  const chosen = selectable.filter((i) => picked.has(i.key));
  const apply = () => {
    const overwrite = chosen.filter((i) => i.kind === "mismatch").map((i) => i.key);
    onApply(chosen.map((i) => i.key), overwrite, message);
  };
  const allSafe = () => setPicked(new Set(selectable.filter((i) => i.kind !== "mismatch").map((i) => i.key)));

  return (
    <section className="lite-list" aria-label={title}>
      <header className="lite-list-head">
        <button className="key" onClick={onBack} disabled={busy}>
          ← Back
        </button>
        <h2>
          {title} <span className="muted">· {items.length}</span>
        </h2>
        <button className="key" onClick={allSafe} disabled={busy || !selectable.length}>
          Select all
        </button>
        <button className="key" onClick={() => setPicked(new Set())} disabled={busy || !chosen.length}>
          Select none
        </button>
      </header>
      {items.length === 0 ? (
        <div className="lite-empty">
          <strong>Nothing to {direction}</strong>
          <span>{direction === "upload" ? "Your computer matches Google Drive." : "You have the latest from Google Drive."}</span>
        </div>
      ) : (
        <ul className="lite-rows">
          {items.map((item) => (
            <li key={item.key} className={item.blocked ? "blocked" : ""}>
              <label>
                <input
                  type="checkbox"
                  checked={picked.has(item.key)}
                  disabled={busy || !!item.blocked}
                  onChange={(e) => toggle(item, e.target.checked)}
                />
                <KindIcon kind={item.kind} />
                <span className="lite-path">
                  {item.path}
                  {subLine(item, direction)}
                </span>
              </label>
            </li>
          ))}
        </ul>
      )}
      <footer className="lite-list-foot">
        <Legend />
        <span style={{ flex: 1 }} />
        {direction === "upload" && (
          <input
            className="lite-message"
            placeholder="What changed? (optional)"
            value={message}
            maxLength={500}
            onChange={(e) => setMessage(e.target.value)}
            disabled={busy}
          />
        )}
        <button className="key key-accent" onClick={apply} disabled={busy || !chosen.length}>
          {busy && <Spinner size={12} />}
          {busy
            ? direction === "upload" ? "Uploading…" : "Downloading…"
            : `${direction === "upload" ? "Upload" : "Download"} ${chosen.length} item${chosen.length === 1 ? "" : "s"}`}
        </button>
      </footer>
    </section>
  );
}
