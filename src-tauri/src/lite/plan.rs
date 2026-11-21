//! What's ready to upload and what's ready to download, CacheCAD-style.
//!
//! Each file is compared three ways: the local copy, the Drive copy, and the
//! state both were in when this computer last uploaded or downloaded it (the
//! index's `local` table). Nothing here changes any file; the plan is only a
//! list to pick from.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::drive::{props, DriveApi, RemoteFile};
use crate::error::Result;
use crate::index::{LocalState, PathMap};
use crate::names::is_temp_file;
use crate::sync::{localfs, shown, Vault};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Not on the other side yet.
    New,
    /// Changed on one side only; replaces the other side's copy.
    Revision,
    /// Deleted on one side; deletes the other side's copy.
    Delete,
    /// Changed on both sides (or deleted on one, changed on the other): can't
    /// tell which is newer. Never ticked by default.
    Mismatch,
    /// Renamed or moved on Drive; moves the local copy to match.
    Move,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeItem {
    /// Stable key the UI sends back to apply this item.
    pub key: String,
    pub kind: Kind,
    /// Path in the project (where the file ends up, for downloads and moves).
    pub path: String,
    /// Moves: where the local copy is now.
    pub from_path: Option<String>,
    pub file_id: Option<String>,
    /// Ticked by default.
    pub checked: bool,
    /// Uploads: why this can't be uploaded (checked out in Permafrost, being pushed).
    pub blocked: Option<String>,
    /// Who last changed the Drive copy, and when.
    pub modified_by: Option<String>,
    pub modified_time: Option<String>,
    /// A short explanation for Mismatch rows.
    pub detail: Option<String>,
}

/// A file found identical on both sides with no record of syncing it (e.g. a
/// local folder linked to Drive for the first time): adopted as in sync.
#[derive(Debug, Clone, PartialEq)]
pub struct Adopt {
    pub file_id: String,
    pub rel: PathBuf,
    pub rev: String,
    pub md5: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LitePlan {
    pub upload: Vec<ChangeItem>,
    pub download: Vec<ChangeItem>,
    /// Bookkeeping applied without asking: identical files to adopt, and records
    /// of files gone from both sides.
    #[serde(skip)]
    pub adopt: Vec<Adopt>,
    #[serde(skip)]
    pub forget: Vec<String>,
}

fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn item(key: String, kind: Kind, path: &Path) -> ChangeItem {
    ChangeItem {
        key,
        kind,
        path: shown(path),
        from_path: None,
        file_id: None,
        checked: kind != Kind::Mismatch,
        blocked: None,
        modified_by: None,
        modified_time: None,
        detail: None,
    }
}

fn with_remote(mut i: ChangeItem, f: &RemoteFile) -> ChangeItem {
    i.file_id = Some(f.id.clone());
    i.modified_by = f.last_modifier.clone();
    i.modified_time = Some(f.modified_time.clone());
    i
}

fn mismatch(i: ChangeItem, detail: &str) -> ChangeItem {
    ChangeItem { detail: Some(detail.to_string()), checked: false, ..i }
}

impl<D: DriveApi> Vault<D> {
    pub(crate) fn lite_modified(&self, l: &LocalState) -> Result<bool> {
        let fp = l.synced_size.zip(l.synced_mtime);
        localfs::is_modified(&self.abs(Path::new(&l.rel_path)), l.synced_md5.as_deref(), fp)
    }

    /// Why an upload of this Drive file would be refused, if it would be.
    fn upload_blocker(&self, f: &RemoteFile) -> Result<Option<String>> {
        if let Some(l) = f.lock() {
            return Ok(Some(format!("Checked out in Permafrost by {}", l.display_owner())));
        }
        if f.read_only {
            return Ok(Some("Read-only on Drive".into()));
        }
        if let Some(cs) = f.prop(props::PENDING_CHANGESET) {
            if self.pending_blocks(cs)? {
                return Ok(Some("Someone is uploading this right now".into()));
            }
        }
        Ok(None)
    }

    pub fn lite_plan(&self) -> Result<LitePlan> {
        let all = self.index.all_files()?;
        let map = PathMap::build(&self.root_id, &all);
        let by_id: HashMap<&str, &RemoteFile> = all.iter().map(|f| (f.id.as_str(), f)).collect();
        let locals = self.index.all_local()?;
        let tracked: HashSet<&str> = locals.iter().map(|l| l.file_id.as_str()).collect();
        // Paths on disk that belong to a known file, so they aren't listed as new.
        let mut claimed: HashSet<String> = locals.iter().map(|l| path_key(Path::new(&l.rel_path))).collect();
        let mut plan = LitePlan::default();

        for l in &locals {
            let old = PathBuf::from(&l.rel_path);
            let exists = self.abs(&old).is_file();
            let changed_here = exists && self.lite_modified(l)?;
            let remote = by_id.get(l.file_id.as_str()).copied().filter(|f| map.contains(&f.id));
            let Some(f) = remote else {
                // Deleted on Drive (or moved out of the project).
                match (exists, changed_here) {
                    (false, _) => plan.forget.push(l.file_id.clone()),
                    (true, false) => plan.download.push(item(format!("d:{}", l.file_id), Kind::Delete, &old)),
                    (true, true) => {
                        let why = "Deleted on Drive, changed on this computer";
                        plan.upload.push(mismatch(item(format!("u:{}", l.file_id), Kind::Mismatch, &old), why));
                        plan.download.push(mismatch(item(format!("d:{}", l.file_id), Kind::Mismatch, &old), why));
                    }
                }
                continue;
            };
            let new = map.path(&f.id).expect("contains checked").to_path_buf();
            claimed.insert(path_key(&new));
            let in_progress = f.prop(props::PENDING_CHANGESET).map(|cs| self.pending_blocks(cs)).transpose()?.unwrap_or(false);
            let changed_there = !in_progress && f.head_revision_id.is_some() && f.head_revision_id != l.synced_rev;
            let blocked = self.upload_blocker(f)?;
            let up = |kind| ChangeItem { blocked: blocked.clone(), ..with_remote(item(format!("u:{}", f.id), kind, &old), f) };
            let down = |kind, path: &Path| with_remote(item(format!("d:{}", f.id), kind, path), f);

            if !exists {
                if changed_there {
                    let why = "Deleted on this computer, changed on Drive";
                    plan.upload.push(mismatch(up(Kind::Mismatch), why));
                    plan.download.push(mismatch(down(Kind::Mismatch, &new), why));
                } else if !in_progress {
                    plan.upload.push(up(Kind::Delete));
                }
                continue;
            }
            match (changed_here, changed_there) {
                (true, true) => {
                    let why = "Changed on this computer and on Drive";
                    plan.upload.push(mismatch(up(Kind::Mismatch), why));
                    plan.download.push(mismatch(ChangeItem { from_path: (new != old).then(|| shown(&old)), ..down(Kind::Mismatch, &new) }, why));
                }
                (true, false) => plan.upload.push(up(Kind::Revision)),
                (false, true) => {
                    plan.download.push(ChangeItem { from_path: (new != old).then(|| shown(&old)), ..down(Kind::Revision, &new) });
                }
                (false, false) => {}
            }
            if new != old && !changed_there && !in_progress {
                plan.download.push(ChangeItem { key: format!("m:{}", f.id), from_path: Some(shown(&old)), ..down(Kind::Move, &new) });
            }
        }

        // On Drive, never synced to this computer.
        for f in &all {
            if f.is_folder() || tracked.contains(f.id.as_str()) || !map.contains(&f.id) {
                continue;
            }
            let Some(rev) = f.head_revision_id.clone() else { continue };
            if let Some(cs) = f.prop(props::PENDING_CHANGESET) {
                if self.pending_blocks(cs)? {
                    continue; // still being uploaded by someone
                }
            }
            let rel = map.path(&f.id).expect("contains checked").to_path_buf();
            let abs = self.abs(&rel);
            if !abs.is_file() {
                plan.download.push(with_remote(item(format!("d:{}", f.id), Kind::New, &rel), f));
                continue;
            }
            // Already here under the same name: identical means in sync.
            let local_md5 = localfs::md5_file(&abs)?;
            if f.md5.as_deref() == Some(local_md5.as_str()) {
                plan.adopt.push(Adopt { file_id: f.id.clone(), rel, rev, md5: local_md5 });
                continue;
            }
            let why = "On Drive and on this computer, with different contents";
            let blocked = self.upload_blocker(f)?;
            plan.upload.push(mismatch(ChangeItem { blocked, ..with_remote(item(format!("u:{}", f.id), Kind::Mismatch, &rel), f) }, why));
            plan.download.push(mismatch(with_remote(item(format!("d:{}", f.id), Kind::Mismatch, &rel), f), why));
        }

        // On this computer, not on Drive.
        let mut fresh = Vec::new();
        collect_new(&self.root, Path::new(""), &map, &claimed, &mut fresh)?;
        for rel in fresh {
            plan.upload.push(item(format!("n:{}", shown(&rel)), Kind::New, &rel));
        }

        plan.upload.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        plan.download.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        Ok(plan)
    }

    /// Record adopted files as in sync and drop records of files gone from both sides.
    pub fn lite_apply_bookkeeping(&self, plan: &LitePlan) -> Result<()> {
        for a in &plan.adopt {
            let fp = localfs::fingerprint(&self.abs(&a.rel));
            self.index.set_local(&LocalState {
                file_id: a.file_id.clone(),
                rel_path: a.rel.to_string_lossy().to_string(),
                synced_rev: Some(a.rev.clone()),
                synced_md5: Some(a.md5.clone()),
                synced_size: fp.map(|x| x.0),
                synced_mtime: fp.map(|x| x.1),
            })?;
        }
        for id in &plan.forget {
            self.index.remove_local(id)?;
        }
        Ok(())
    }
}

/// Files on disk in the project that no Drive file or sync record accounts for.
fn collect_new(root: &Path, rel: &Path, map: &PathMap, claimed: &HashSet<String>, out: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(dir) = std::fs::read_dir(root.join(rel)) else { return Ok(()) };
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || is_temp_file(&name) {
            continue;
        }
        let child = rel.join(&name);
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect_new(root, &child, map, claimed, out)?;
        } else if kind.is_file() && map.id_for(&child).is_none() && !claimed.contains(&path_key(&child)) {
            out.push(child);
        }
    }
    Ok(())
}
