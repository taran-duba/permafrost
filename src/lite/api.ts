// Typed wrappers around Permafrost Lite's own commands (src-tauri/src/app/lite_ops.rs).
// Shared commands (status, sign in, projects) come from ../api.
import { invoke } from "@tauri-apps/api/core";

export type Kind = "new" | "revision" | "delete" | "mismatch" | "move";

export interface ChangeItem {
  key: string;
  kind: Kind;
  path: string;
  fromPath: string | null;
  fileId: string | null;
  /** Ticked by default (Mismatch rows never are). */
  checked: boolean;
  /** Uploads only: why it can't be uploaded right now. */
  blocked: string | null;
  modifiedBy: string | null;
  modifiedTime: string | null;
  detail: string | null;
}

export interface LitePlan {
  upload: ChangeItem[];
  download: ChangeItem[];
}

export interface DownloadReport {
  downloaded: number;
  moved: number;
  deleted: number;
  backups: string[];
  skipped: string[];
}

export interface SkippedFile {
  rel_path: string;
  reason: string;
}

export interface UploadReport {
  changeset: string | null;
  uploaded: number;
  deleted: number;
  skipped: SkippedFile[];
}

export interface LiteConfig {
  liteRoot: string | null;
  defaultParent: string | null;
  defaultParentName: string | null;
}

export const lite = {
  config: () => invoke<LiteConfig>("get_config"),
  changes: () => invoke<LitePlan>("lite_changes"),
  download: (keys: string[]) => invoke<DownloadReport>("lite_download", { keys }),
  upload: (keys: string[], overwrite: string[], message: string) =>
    invoke<UploadReport>("lite_upload", { keys, overwrite, message }),
  setRoot: (root: string) => invoke<void>("lite_set_root", { root }),
  setDefaultParent: (input: string) => invoke<string>("lite_set_default_parent", { input }),
  openDefaultParent: () => invoke<void>("lite_open_default_parent"),
  revealBase: () => invoke<void>("lite_reveal_base"),
  revealBackups: () => invoke<void>("lite_reveal_backups"),
  addProject: (args: { link: string | null; newName: string | null; localFolder: string | null }) =>
    invoke<void>("lite_add_project", args),
};

/** The last part of a path, for labels. */
export function baseName(p: string | null | undefined): string {
  if (!p) return "";
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? p;
}
