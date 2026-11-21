import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, Status } from "../api";
import { Toasts, useToasts } from "../Toasts";
import { Spinner } from "../Spinner";
import { useUpdater } from "../updater";
import { ChangeList } from "./ChangeList";
import { FirstRun, NewProject, Settings } from "./Dialogs";
import { Sidebar } from "./Sidebar";
import { lite, LiteConfig, LitePlan } from "./api";
import "../App.css";
import "./lite.css";

type View = "home" | "upload" | "download";

function Logo() {
  return (
    <svg className="lite-logo" viewBox="0 0 1024 1024" aria-hidden>
      <rect x="64" y="64" width="896" height="896" rx="200" fill="#0b141b" stroke="#28343f" strokeWidth="16" />
      <g fill="none" strokeLinecap="round" strokeLinejoin="round" strokeWidth="84">
        <path d="M384 760V280M240 420l144-144 144 144" stroke="#65d0dc" />
        <path d="M640 264v480M496 604l144 144 144-144" stroke="#e5edf1" />
      </g>
    </svg>
  );
}

export default function LiteApp() {
  const [status, setStatus] = useState<Status | null>(null);
  const [config, setConfig] = useState<LiteConfig | null>(null);
  const [plan, setPlan] = useState<LitePlan | null>(null);
  const [view, setView] = useState<View>("home");
  const [busy, setBusy] = useState(false);
  const [loadingProject, setLoadingProject] = useState<string | null>(null);
  const [dialog, setDialog] = useState<"settings" | "new" | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const toasts = useToasts();
  const updater = useUpdater(!import.meta.env.DEV);
  const planBusy = useRef(false);

  useEffect(() => {
    document.documentElement.classList.add("lite-root");
    document.title = "Permafrost Lite";
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const [s, c] = await Promise.all([api.status(), lite.config()]);
      setStatus(s);
      setConfig(c);
      if (!s.signedIn) setPlan(null);
      return s;
    } catch (e) {
      toasts.error(errorText(e));
      return null;
    }
  }, [toasts.error]);

  const refreshPlan = useCallback(async () => {
    if (planBusy.current) return;
    planBusy.current = true;
    try {
      setPlan(await lite.changes());
    } catch (e) {
      // Not signed in / no project yet: nothing to list.
      if (!/sign in/i.test(errorText(e))) toasts.error(errorText(e));
    } finally {
      planBusy.current = false;
    }
  }, [toasts.error]);

  const refreshAll = useCallback(async () => {
    const s = await refreshStatus();
    if (s?.signedIn) await refreshPlan();
  }, [refreshStatus, refreshPlan]);

  // Until a project is open, re-ask every couple of seconds (sign-in finishes in the browser).
  const projectOpen = !!status?.signedIn;
  useEffect(() => {
    if (projectOpen) return;
    const t = setInterval(refreshStatus, 2000);
    return () => clearInterval(t);
  }, [projectOpen, refreshStatus]);

  useEffect(() => {
    refreshAll()
      .then(() => api.frontendReady())
      .then((smoke) => (smoke ? import("./smokeDriver").then((m) => m.runLiteSmokeUi()) : undefined))
      .catch(() => {});
    api
      .takeNewCrashReport()
      .then((p) => p && toasts.warn("Permafrost Lite closed unexpectedly last time. A crash report was saved: Settings › Open logs folder."))
      .catch(() => {});
    const onFocus = () => void refreshPlan();
    window.addEventListener("focus", onFocus);
    const unlisten = [
      listen("vault-changed", () => void refreshAll()),
      listen("sync-report", () => {
        setSyncError(null);
        void refreshPlan();
      }),
      listen<string>("sync-error", (e) => setSyncError(e.payload)),
    ];
    return () => {
      window.removeEventListener("focus", onFocus);
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [refreshAll, refreshPlan, toasts.warn]);

  const download = async (keys: string[]) => {
    setBusy(true);
    try {
      const r = await lite.download(keys);
      const parts = [
        r.downloaded && `${r.downloaded} downloaded`,
        r.moved && `${r.moved} moved`,
        r.deleted && `${r.deleted} deleted (kept in the app's trash)`,
      ].filter(Boolean);
      if (parts.length) toasts.info(parts.join(", ") + ".");
      if (r.backups.length) toasts.info(`Saved ${r.backups.length} of your changed file(s) to the backups folder first.`);
      r.skipped.forEach((s) => toasts.warn(`Not downloaded: ${s}`));
      setView("home");
    } catch (e) {
      toasts.error(`Nothing was changed on this computer: ${errorText(e)}`);
    } finally {
      setBusy(false);
      void refreshAll();
    }
  };

  const upload = async (keys: string[], overwrite: string[], message: string) => {
    setBusy(true);
    try {
      const r = await lite.upload(keys, overwrite, message);
      const parts = [r.uploaded && `${r.uploaded} uploaded`, r.deleted && `${r.deleted} deleted from Drive`].filter(Boolean);
      if (parts.length) toasts.info(parts.join(", ") + ".");
      r.skipped.forEach((s) => toasts.warn(`${s.rel_path}: ${s.reason}`));
      setView("home");
    } catch (e) {
      toasts.error(`Upload stopped: ${errorText(e)}. Anything already sent finishes automatically.`);
    } finally {
      setBusy(false);
      void refreshAll();
    }
  };

  const installUpdate = async () => {
    try {
      await updater.install();
    } catch (e) {
      toasts.error(errorText(e));
    }
  };

  if (!status)
    return (
      <div className="lite">
        <div className="lite-empty" style={{ gridColumn: "1 / -1" }}>
          <span>
            <Spinner size={16} /> Starting Permafrost Lite…
          </span>
        </div>
      </div>
    );

  const googleSignedIn = !!status.account || status.demo;
  const ups = plan?.upload ?? [];
  const downs = plan?.download ?? [];
  const attention = (items: typeof ups) => items.filter((i) => i.kind === "mismatch" || i.blocked).length;

  return (
    <div className="lite">
      <Sidebar
        status={status}
        loading={loadingProject}
        onSettings={() => setDialog("settings")}
        onNewProject={() => setDialog("new")}
        onChanged={(opening) => {
          if (opening !== undefined) setLoadingProject(opening);
          setView("home");
          void refreshAll();
        }}
        toasts={toasts}
      />
      <main className="lite-main">
        <header className="lite-head">
          <h1 className="lite-title">
            {status.signedIn ? status.folderName : "Permafrost Lite"}
            {status.demo && <span className="pill pill-warn lite-badge">demo</span>}
          </h1>
          {updater.version ? (
            <button className="pill pill-info pill-button" onClick={installUpdate} disabled={updater.installing}>
              {updater.installing ? "Updating…" : `Update to ${updater.version}`}
            </button>
          ) : syncError ? (
            <span className="pill pill-bad" title={syncError}>
              Can't reach Drive · retrying
            </span>
          ) : null}
          <Logo />
        </header>

        {!googleSignedIn ? (
          <div className="lite-empty">
            <strong>Sign in with Google to get started</strong>
            <span>Use the button at the top left. Your projects sync with folders on Google Drive.</span>
            <button className="link" onClick={() => api.startDemo().then(refreshAll).catch((e) => toasts.error(errorText(e)))}>
              Or try the demo first
            </button>
          </div>
        ) : !status.signedIn ? (
          <div className="lite-empty">
            {loadingProject ? (
              <span>
                <Spinner size={14} /> Loading project…
              </span>
            ) : (
              <>
                <strong>{status.projects.length ? "Double-click a project to load it" : "Add your first project"}</strong>
                <span>{status.projects.length ? "Your projects are listed on the left." : "Click New Project and paste a Google Drive folder link."}</span>
              </>
            )}
          </div>
        ) : view === "home" ? (
          <div className="lite-panels">
            <section className="lite-panel" aria-label="Upload Changes">
              <button className="key" onClick={() => setView("upload")}>
                Upload Changes
              </button>
              <div className="lite-count">
                {plan ? ups.length : <Spinner size={20} />}
                <small>Files ready to upload</small>
              </div>
              <p className="lite-panel-note">{attention(ups) ? `${attention(ups)} need a look before uploading` : ""}</p>
              <button className="key" onClick={() => api.revealRoot().catch((e) => toasts.error(errorText(e)))}>
                Open Local Folder
              </button>
            </section>
            <section className="lite-panel" aria-label="Download Changes">
              <button className="key" onClick={() => setView("download")}>
                Download Changes
              </button>
              <div className="lite-count">
                {plan ? downs.length : <Spinner size={20} />}
                <small>Files ready to download</small>
              </div>
              <p className="lite-panel-note">{attention(downs) ? `${attention(downs)} changed on both sides` : ""}</p>
              <button
                className="key"
                disabled={status.demo}
                title={status.demo ? "The demo has no Google Drive to open" : undefined}
                onClick={() => api.openInDrive(null).catch((e) => toasts.error(errorText(e)))}
              >
                Open Google Drive
              </button>
            </section>
          </div>
        ) : (
          <ChangeList
            key={view}
            direction={view}
            items={view === "upload" ? ups : downs}
            busy={busy}
            onBack={() => setView("home")}
            onApply={(keys, overwrite, message) => (view === "upload" ? upload(keys, overwrite, message) : download(keys))}
          />
        )}
      </main>

      {config && !config.liteRoot && <FirstRun status={status} onDone={refreshAll} toasts={toasts} />}
      {dialog === "new" && (
        <NewProject
          status={status}
          config={config}
          onClose={() => setDialog(null)}
          onAdded={() => {
            setDialog(null);
            void refreshAll();
          }}
          toasts={toasts}
        />
      )}
      {dialog === "settings" && (
        <Settings
          status={status}
          config={config}
          updater={updater}
          onInstallUpdate={installUpdate}
          onClose={() => setDialog(null)}
          onChanged={refreshAll}
          toasts={toasts}
        />
      )}
      <Toasts state={toasts} />
    </div>
  );
}
