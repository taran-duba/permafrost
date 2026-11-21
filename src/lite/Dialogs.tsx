import { ReactNode, useState } from "react";
import { Dialog } from "radix-ui";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { api, errorText, Status } from "../api";
import { ToastApi } from "../Toasts";
import { Spinner } from "../Spinner";
import { UpdaterState } from "../updater";
import { baseName, lite, LiteConfig } from "./api";

function Modal({ open, onClose, title, children }: { open: boolean; onClose: () => void; title: string; children: ReactNode }) {
  return (
    <Dialog.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="lite-overlay" />
        <Dialog.Content className="lite-dialog" aria-describedby={undefined}>
          <Dialog.Title>{title}</Dialog.Title>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

async function chooseFolder(defaultPath: string | undefined): Promise<string | null> {
  const picked = await pickFolder({ directory: true, multiple: false, defaultPath });
  return typeof picked === "string" ? picked : null;
}

/** CacheCAD's first-run question: where do project folders go? */
export function FirstRun({ status, onDone, toasts }: { status: Status; onDone: () => void; toasts: ToastApi }) {
  const [busy, setBusy] = useState(false);
  const use = async (root: string | null) => {
    if (!root) return;
    setBusy(true);
    try {
      await lite.setRoot(root);
      onDone();
    } catch (e) {
      toasts.error(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal open onClose={() => {}} title="Setup">
      <p>
        Looks like this is a new install of Permafrost Lite. Choose where to keep the project folders you sync. You can
        change this later in Settings. The default is:
      </p>
      <p>
        <code>{status.baseDir}</code>
      </p>
      <p>Do you want to use this location?</p>
      <div className="lite-actions">
        <button className="key" disabled={busy} onClick={async () => use(await chooseFolder(status.baseDir))}>
          No, choose another
        </button>
        <button className="key key-accent" disabled={busy} onClick={() => use(status.baseDir)}>
          Yes
        </button>
      </div>
    </Modal>
  );
}

/** New Project: link a Drive folder or create one, then pick the local folder. */
export function NewProject({
  status,
  config,
  onClose,
  onAdded,
  toasts,
}: {
  status: Status;
  config: LiteConfig | null;
  onClose: () => void;
  onAdded: () => void;
  toasts: ToastApi;
}) {
  const [step, setStep] = useState<"drive" | "local">("drive");
  const [link, setLink] = useState("");
  const [newName, setNewName] = useState<string | null>(null);
  const [local, setLocal] = useState("");
  const [busy, setBusy] = useState(false);

  const add = async () => {
    setBusy(true);
    try {
      await lite.addProject({
        link: newName === null ? link.trim() : null,
        newName: newName === null ? null : newName.trim(),
        localFolder: local.trim() || null,
      });
      toasts.info("Project added. Its files are listed under Download Changes.");
      onAdded();
    } catch (e) {
      toasts.error(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open onClose={onClose} title="New Project">
      {step === "drive" && newName === null && (
        <>
          <p>Paste a link to an existing folder on Google Drive.</p>
          <div className="lite-field">
            <input
              autoFocus
              value={link}
              onChange={(e) => setLink(e.target.value)}
              placeholder="https://drive.google.com/drive/folders/…"
              onKeyDown={(e) => e.key === "Enter" && link.trim() && setStep("local")}
            />
            <button className="key key-accent" disabled={!link.trim()} onClick={() => setStep("local")}>
              Ok
            </button>
          </div>
          <p className="lite-or">OR</p>
          <div className="lite-field">
            <span style={{ flex: 1 }}>
              Create a new folder on Google Drive
              {config?.defaultParentName ? ` in ${config.defaultParentName}` : ""}.
            </span>
            <button className="key" onClick={() => lite.openDefaultParent().catch((e) => toasts.error(errorText(e)))}>
              Open Drive
            </button>
            <button
              className="key"
              onClick={() =>
                config?.defaultParent
                  ? setNewName("")
                  : toasts.warn("Choose a default Drive location in Settings first, or paste a folder link.")
              }
            >
              New
            </button>
          </div>
        </>
      )}
      {step === "drive" && newName !== null && (
        <>
          <p>Name the new folder. It's created in {config?.defaultParentName ?? "your default Drive location"}.</p>
          <div className="lite-field">
            <input autoFocus value={newName} maxLength={200} onChange={(e) => setNewName(e.target.value)} placeholder="2027 Robot" />
            <button className="key key-accent" disabled={!newName.trim()} onClick={() => setStep("local")}>
              Next
            </button>
          </div>
        </>
      )}
      {step === "local" && (
        <>
          <p>
            Choose the local project folder inside <code>{status.baseDir}</code>. Leave it empty to name it after the Drive
            folder.
          </p>
          <div className="lite-field">
            <input autoFocus value={local} onChange={(e) => setLocal(e.target.value)} placeholder="Same name as the Drive folder" />
            <button
              className="key"
              onClick={async () => {
                const p = await chooseFolder(status.baseDir).catch(() => null);
                if (p) setLocal(p);
              }}
            >
              Browse…
            </button>
          </div>
        </>
      )}
      <div className="lite-actions">
        <button className="key" onClick={onClose} disabled={busy}>
          Cancel
        </button>
        {step === "local" && (
          <button className="key key-accent" onClick={add} disabled={busy}>
            {busy && <Spinner size={12} />}
            {busy ? "Adding…" : "Add project"}
          </button>
        )}
      </div>
    </Modal>
  );
}

export function Settings({
  status,
  config,
  updater,
  onInstallUpdate,
  onClose,
  onChanged,
  toasts,
}: {
  status: Status;
  config: LiteConfig | null;
  updater: UpdaterState;
  onInstallUpdate: () => void;
  onClose: () => void;
  onChanged: () => void;
  toasts: ToastApi;
}) {
  const [parentInput, setParentInput] = useState<string | null>(null);
  const [checkResult, setCheckResult] = useState<string | null>(null);
  const run = (p: Promise<unknown>) => p.then(onChanged).catch((e) => toasts.error(errorText(e)));

  const changeRoot = async () => {
    const p = await chooseFolder(status.baseDir).catch(() => null);
    if (p) await run(lite.setRoot(p));
  };
  const saveParent = async () => {
    try {
      const name = await lite.setDefaultParent(parentInput ?? "");
      toasts.info(`New projects will be created in ${name}.`);
      setParentInput(null);
      onChanged();
    } catch (e) {
      toasts.error(errorText(e));
    }
  };
  const check = async () => {
    try {
      setCheckResult((await updater.checkNow()) === "available" ? null : "You're on the latest version.");
    } catch (e) {
      setCheckResult(`Couldn't check: ${errorText(e)}`);
    }
  };

  return (
    <Modal open onClose={onClose} title="Settings">
      <section>
        <h3>Root folder</h3>
        <p className="muted small">Where project folders go. Projects you already added stay where they are.</p>
        <div className="lite-field">
          <code style={{ flex: 1 }}>{status.baseDir}</code>
          <button className="key" onClick={changeRoot}>
            Change
          </button>
          <button className="key" onClick={() => lite.revealBase().catch((e) => toasts.error(errorText(e)))}>
            Open
          </button>
        </div>
      </section>
      <section>
        <h3>Default Drive location</h3>
        <p className="muted small">
          Used when you create a new project and haven't linked an existing Drive folder. You can always link projects to
          any Drive folder.
        </p>
        {parentInput === null ? (
          <div className="lite-field">
            <span style={{ flex: 1 }}>{config?.defaultParentName ?? <span className="muted">Not set</span>}</span>
            <button className="key" onClick={() => setParentInput("")} disabled={status.demo}>
              Change
            </button>
            <button className="key" onClick={() => lite.openDefaultParent().catch((e) => toasts.error(errorText(e)))}>
              Open
            </button>
          </div>
        ) : (
          <div className="lite-field">
            <input
              autoFocus
              value={parentInput}
              onChange={(e) => setParentInput(e.target.value)}
              placeholder="Paste a Drive folder link, or type root for My Drive"
              onKeyDown={(e) => e.key === "Enter" && parentInput.trim() && saveParent()}
            />
            <button className="key key-accent" onClick={saveParent} disabled={!parentInput.trim()}>
              Save
            </button>
            <button className="key" onClick={() => setParentInput(null)}>
              Cancel
            </button>
          </div>
        )}
      </section>
      <section>
        <h3>Account</h3>
        <div className="lite-field">
          <span style={{ flex: 1 }}>{status.demo ? "Demo" : status.account ?? <span className="muted">Not signed in</span>}</span>
        </div>
      </section>
      <section>
        <h3>Updates</h3>
        <div className="lite-field">
          {updater.version ? (
            <button className="key key-accent" onClick={onInstallUpdate} disabled={updater.installing}>
              {updater.installing ? "Updating…" : `Update to ${updater.version} · restart`}
            </button>
          ) : (
            <button className="key" onClick={check} disabled={updater.checking}>
              {updater.checking ? "Checking…" : "Check for updates"}
            </button>
          )}
          {checkResult && <span className="muted small">{checkResult}</span>}
        </div>
      </section>
      <section>
        <h3>Troubleshooting</h3>
        <div className="lite-field">
          <button className="key" onClick={() => api.openLogs().catch((e) => toasts.error(errorText(e)))}>
            Open logs folder
          </button>
          {status.signedIn && (
            <button className="key" onClick={() => lite.revealBackups().catch((e) => toasts.error(errorText(e)))}>
              Open backups folder
            </button>
          )}
        </div>
        <p className="muted small">
          Before a download replaces or deletes a file you changed, your copy is saved to this project's backups folder.
        </p>
        <div>
        </div>
      </section>
      <div className="lite-actions">
        <button className="key key-accent" onClick={onClose}>
          Done
        </button>
      </div>
    </Modal>
  );
}

export { baseName };
