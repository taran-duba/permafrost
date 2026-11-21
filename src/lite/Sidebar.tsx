import { useState } from "react";
import { api, errorText, Status } from "../api";
import { ToastApi } from "../Toasts";
import { Spinner } from "../Spinner";
import { baseName, lite } from "./api";

function GearIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden>
      <circle cx="10" cy="10" r="2.6" />
      <path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden>
      <path d="M2.5 5.5h5l1.5 2h8.5v8h-15z" />
    </svg>
  );
}

/** CacheCAD's left column: account and settings, root folder, project list, add/remove. */
export function Sidebar({
  status,
  loading,
  onSettings,
  onNewProject,
  onChanged,
  toasts,
}: {
  status: Status;
  /** A project is being loaded (double-clicked). */
  loading: string | null;
  onSettings: () => void;
  onNewProject: () => void;
  onChanged: (opening?: string | null) => void;
  toasts: ToastApi;
}) {
  const [selected, setSelected] = useState<string | null>(null);
  const [signingIn, setSigningIn] = useState(false);
  const googleSignedIn = !!status.account || status.demo;

  const signIn = async () => {
    setSigningIn(true);
    try {
      await api.signIn();
    } catch (e) {
      toasts.error(errorText(e));
    } finally {
      setSigningIn(false);
      onChanged();
    }
  };
  const signOut = async () => {
    try {
      await api.signOut();
    } catch (e) {
      toasts.error(errorText(e));
    }
    onChanged();
  };
  const load = async (id: string) => {
    if (loading || status.projects.find((p) => p.id === id)?.active) return;
    onChanged(id);
    try {
      await api.openProject(id);
    } catch (e) {
      toasts.error(errorText(e));
    }
    onChanged(null);
  };
  const remove = async () => {
    const p = status.projects.find((x) => x.id === selected);
    if (!p) return;
    if (!window.confirm(`Remove ${p.name} from this list?\n\nNothing is deleted: its files stay in ${p.root || "its folder"} and on Google Drive.`)) return;
    try {
      await api.removeProject(p.id);
      setSelected(null);
    } catch (e) {
      toasts.error(errorText(e));
    }
    onChanged();
  };

  return (
    <aside className="lite-side">
      <div className="lite-row">
        {googleSignedIn ? (
          <button className="key" onClick={signOut} disabled={status.demo}>
            Sign out
          </button>
        ) : (
          <button
            className="key key-google"
            onClick={signIn}
            disabled={signingIn || status.restoring || !status.signInAvailable}
            title={status.signInAvailable ? undefined : "This build has no Google sign-in"}
          >
            {(signingIn || status.restoring) && <Spinner size={12} />}
            {status.restoring ? "Signing in…" : "Sign in with Google"}
          </button>
        )}
        <button className="key" onClick={onSettings}>
          <GearIcon /> Settings
        </button>
      </div>
      <button className="key" onClick={() => lite.revealBase().catch((e) => toasts.error(errorText(e)))} title={status.baseDir}>
        <FolderIcon /> {baseName(status.baseDir) || "Root folder"}
      </button>
      <ul className="lite-projects" role="listbox" aria-label="Projects">
        {status.projects.map((p) => (
          <li key={p.id}>
            <button
              className={`project ${selected === p.id ? "selected" : ""} ${p.active ? "active" : ""}`}
              role="option"
              aria-selected={selected === p.id}
              title={p.root ? `${p.root}\nDouble-click to load` : "Double-click to load"}
              onClick={() => setSelected(p.id)}
              onDoubleClick={() => load(p.id)}
              onKeyDown={(e) => e.key === "Enter" && load(p.id)}
            >
              {loading === p.id && <Spinner size={11} />} {p.name}
            </button>
          </li>
        ))}
        {googleSignedIn && status.projects.length === 0 && <li className="lite-hint">No projects yet. Click New Project.</li>}
        {status.projects.length > 0 && !status.signedIn && !loading && <li className="lite-hint">Double-click a project to load it.</li>}
      </ul>
      <div className="lite-row">
        <button className="key" onClick={onNewProject} disabled={!googleSignedIn}>
          New Project
        </button>
        <button className="key" onClick={remove} disabled={!selected || status.demo}>
          Remove Project
        </button>
      </div>
    </aside>
  );
}
