//! Atomic check-in (the fix for broken mates).
//!
//! Drive has no transactions, so a check-in is a small protocol:
//!  1. Verify every file is checked out by me and still at the revision I
//!     started from. Nothing is uploaded unless all files pass.
//!  2. Journal the plan locally, then upload parts, then assemblies, then
//!     drawings. Each upload is tagged `permafrostPending=<changeset>`, and other
//!     clients ignore tagged revisions.
//!  3. Write the manifest `.permafrost/changesets/<id>.json`. That write is the
//!     commit; from then on, other clients pull all files together.
//!  4. Mark revisions keep-forever, clear tags, and release or keep the locks.
//!
//! A crash at any step resumes from the journal, and the locks held the whole
//! time mean nobody else could touch the files meanwhile.
//!
//! Permafrost Lite pushes the same way without locks (`Journal::unlocked`):
//! each file is re-checked right before its upload, and a file someone else
//! locked or changed meanwhile is left out of the push instead of overwritten.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::crawl::MANIFEST_PROP;
use super::localfs;
use super::Vault;
use crate::drive::{props, DriveApi, MetadataPatch, RemoteFile};
use crate::error::{Error, Result};
use crate::index::{LocalState, Manifest, META_FOLDER};
use crate::names::{cad_kind, CadKind};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PushRequest {
    /// Existing files, already checked out by me.
    pub files: Vec<String>,
    /// New local files (relative to the vault root) to add to Drive.
    pub new_files: Vec<PathBuf>,
    pub message: String,
    /// Keep the files checked out after the check-in.
    pub keep_locked: bool,
    /// Files or folders to rename/move as part of this push: (file id, new path in the vault).
    /// Files must be checked out by me; folders need no lock.
    #[serde(default)]
    pub moves: Vec<(String, PathBuf)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JournalItem {
    pub(crate) file_id: Option<String>,
    pub(crate) rel_path: PathBuf,
    pub(crate) uploaded_rev: Option<String>,
    /// New files: a key saved before `create_file`, so a resumed push finds
    /// the file it already created instead of creating a duplicate.
    #[serde(default)]
    pub(crate) create_key: Option<String>,
    /// The local fingerprint taken just before uploading. Recorded as the
    /// synced state, so a save made during the upload still shows as a change.
    #[serde(default)]
    pub(crate) pre_fp: Option<(u64, i64)>,
    /// Unlocked pushes: the Drive revision this upload is based on.
    #[serde(default)]
    pub(crate) base_rev: Option<String>,
    /// Unlocked pushes: replace the Drive file even if it changed since `base_rev`.
    #[serde(default)]
    pub(crate) overwrite: bool,
}

impl JournalItem {
    pub(crate) fn new(file_id: Option<String>, rel_path: PathBuf) -> Self {
        Self { file_id, rel_path, uploaded_rev: None, create_key: None, pre_fp: None, base_rev: None, overwrite: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MoveItem {
    file_id: String,
    new_rel: PathBuf,
    is_folder: bool,
    /// Head revision after the move (files only); set once the move is done on Drive.
    rev: Option<String>,
    done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Journal {
    pub(crate) id: String,
    pub(crate) message: String,
    pub(crate) keep_locked: bool,
    pub(crate) created_time: String,
    pub(crate) items: Vec<JournalItem>,
    #[serde(default)]
    moves: Vec<MoveItem>,
    pub(crate) committed: bool,
    /// A Permafrost Lite push: no locks are taken, held or released.
    #[serde(default)]
    pub(crate) unlocked: bool,
    /// Files left out of the push because they changed on Drive or were locked
    /// before their upload started.
    #[serde(default)]
    pub(crate) skipped: Vec<SkippedFile>,
}

impl Journal {
    pub(crate) fn new(message: String, items: Vec<JournalItem>, unlocked: bool) -> Self {
        let mut items = items;
        items.sort_by_key(|i| upload_order(&i.rel_path));
        Self {
            id: ulid::Ulid::new().to_string(),
            message,
            keep_locked: false,
            created_time: chrono::Utc::now().to_rfc3339(),
            items,
            moves: Vec::new(),
            committed: false,
            unlocked,
            skipped: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub rel_path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PushOutcome {
    pub changeset: String,
    pub files: usize,
    /// Files left out (unlocked pushes only): changed on Drive or locked meanwhile.
    pub skipped: Vec<SkippedFile>,
}

fn upload_order(rel: &std::path::Path) -> u8 {
    match cad_kind(&rel.to_string_lossy()) {
        CadKind::Other => 0,
        CadKind::Part => 1,
        CadKind::Assembly => 2,
        CadKind::Drawing => 3,
    }
}

impl<D: DriveApi> Vault<D> {
    pub async fn push(&self, req: PushRequest) -> Result<PushOutcome> {
        if req.files.is_empty() && req.new_files.is_empty() && req.moves.is_empty() {
            return Err(Error::Conflict("nothing to push".into()));
        }
        let map = self.paths()?;
        let mut items = Vec::new();
        // Paths after this push's own moves: a file inside a moved folder is already at its new
        // local path when the push starts.
        let relocations: Vec<(PathBuf, PathBuf)> = req
            .moves
            .iter()
            .filter_map(|(id, new_rel)| Some((map.path(id)?.to_path_buf(), new_rel.clone())))
            .collect();
        let after_moves = |rel: PathBuf| -> PathBuf {
            for (old, new) in &relocations {
                if let Ok(rest) = rel.strip_prefix(old) {
                    return if rest.as_os_str().is_empty() { new.clone() } else { new.join(rest) };
                }
            }
            rel
        };

        // 1. Verify everything before uploading anything.
        for id in &req.files {
            let f = self.drive.get_file(id).await?;
            self.index.apply_remote(std::slice::from_ref(&f), &[])?;
            if !self.i_hold_lock(&f) {
                return Err(match f.lock() {
                    Some(l) => Error::Locked { name: f.name, by: l.display_owner().to_string() },
                    None => Error::NotCheckedOut(f.name),
                });
            }
            let local = self.index.local(id)?.ok_or_else(|| Error::NotFound(f.name.clone()))?;
            if local.synced_rev != f.head_revision_id {
                return Err(Error::Conflict(format!(
                    "{} changed on Drive since you checked it out (was the lock broken?). \
                     Your copy is untouched; save it under another name and get the latest.",
                    f.name
                )));
            }
            let rel = after_moves(map.path(id).ok_or_else(|| Error::NotFound(f.name.clone()))?.to_path_buf());
            if !self.abs(&rel).exists() {
                return Err(Error::NotFound(rel.display().to_string()));
            }
            items.push(JournalItem::new(Some(id.clone()), rel));
        }
        for rel in &req.new_files {
            if map.id_for(rel).is_some() {
                return Err(Error::Conflict(format!("{} is already on Drive", rel.display())));
            }
            self.ensure_can_add(rel)?;
            if !self.abs(rel).is_file() {
                return Err(Error::NotFound(rel.display().to_string()));
            }
            items.push(JournalItem::new(None, rel.clone()));
        }
        items.sort_by_key(|i| upload_order(&i.rel_path));

        let mut moves = Vec::new();
        for (id, new_rel) in &req.moves {
            let f = self.drive.get_file(id).await?;
            self.index.apply_remote(std::slice::from_ref(&f), &[])?;
            if !f.is_folder() && !self.i_hold_lock(&f) {
                return Err(match f.lock() {
                    Some(l) => Error::Locked { name: f.name, by: l.display_owner().to_string() },
                    None => Error::NotCheckedOut(f.name),
                });
            }
            if f.is_folder() {
                self.ensure_can_edit(&f)?;
            }
            self.ensure_can_add(new_rel)?;
            if map.id_for(new_rel).is_some_and(|other| other != id) {
                return Err(Error::Conflict(format!("{} already exists", new_rel.display())));
            }
            moves.push(MoveItem { file_id: id.clone(), new_rel: new_rel.clone(), is_folder: f.is_folder(), rev: None, done: false });
        }

        let j = Journal {
            id: ulid::Ulid::new().to_string(),
            message: req.message,
            keep_locked: req.keep_locked,
            created_time: chrono::Utc::now().to_rfc3339(),
            items,
            moves,
            committed: false,
            unlocked: false,
            skipped: Vec::new(),
        };
        self.run_push(j).await
    }

    /// Journal a verified push, then carry it out.
    pub(crate) async fn run_push(&self, mut j: Journal) -> Result<PushOutcome> {
        self.save_journal(&j)?;
        self.continue_push(&mut j).await
    }

    /// Finish any check-ins interrupted by a crash or lost connection. One
    /// that fails doesn't hold up the others; the first error is returned
    /// once every journal has been tried.
    pub async fn resume_pushes(&self) -> Result<Vec<PushOutcome>> {
        let mut done = Vec::new();
        let mut first_err = None;
        for (id, payload) in self.index.journal()? {
            let result = match serde_json::from_str::<Journal>(&payload) {
                Ok(mut j) => self.continue_push(&mut j).await,
                Err(e) => Err(e.into()),
            };
            match result {
                Ok(out) => done.push(out),
                Err(e) => {
                    tracing::warn!(changeset = %id, %e, "could not finish an interrupted push; will retry");
                    first_err.get_or_insert(e);
                }
            }
        }
        match first_err {
            Some(e) if done.is_empty() => Err(e),
            _ => Ok(done),
        }
    }

    fn save_journal(&self, j: &Journal) -> Result<()> {
        self.index.put_journal(&j.id, &serde_json::to_string(j)?)
    }

    async fn continue_push(&self, j: &mut Journal) -> Result<PushOutcome> {
        // 1b. Renames and moves, tagged so teammates hold them back until the commit.
        for n in 0..j.moves.len() {
            if j.moves[n].done {
                continue;
            }
            let m = j.moves[n].clone();
            let parent = self.ensure_parent_folder(&m.new_rel).await?;
            let name = m.new_rel.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let f = self.drive.get_file(&m.file_id).await?;
            let patch = MetadataPatch::default().set(props::PENDING_CHANGESET, &j.id).relocate(&f, &parent, &name);
            let moved = self.drive.update_metadata(&m.file_id, patch).await?;
            self.index.apply_remote(std::slice::from_ref(&moved), &[])?;
            j.moves[n].rev = moved.head_revision_id.clone();
            j.moves[n].done = true;
            self.save_journal(j)?;
        }

        // 2. Upload whatever isn't uploaded yet.
        let mut n = 0;
        while n < j.items.len() {
            if j.items[n].uploaded_rev.is_some() {
                n += 1;
                continue;
            }
            if j.unlocked {
                if let Some(reason) = self.unlocked_blocker(&j.items[n]).await? {
                    let item = j.items.remove(n);
                    tracing::info!(file = %item.rel_path.display(), %reason, "left out of the push");
                    j.skipped.push(SkippedFile { rel_path: item.rel_path, reason });
                    self.save_journal(j)?;
                    continue;
                }
            }
            let retry = j.items[n].create_key.is_some();
            if j.items[n].file_id.is_none() && !retry {
                j.items[n].create_key = Some(ulid::Ulid::new().to_string());
            }
            j.items[n].pre_fp = localfs::fingerprint(&self.abs(&j.items[n].rel_path));
            self.save_journal(j)?;
            let uploaded = match self.find_created(&j.items[n], retry).await? {
                Some(f) => f,
                None => self.upload_item(&j.id, j.unlocked, &j.items[n]).await?,
            };
            j.items[n].file_id = Some(uploaded.id.clone());
            j.items[n].uploaded_rev = uploaded.head_revision_id.clone();
            self.index.apply_remote(std::slice::from_ref(&uploaded), &[])?;
            self.save_journal(j)?;
            n += 1;
        }

        // 3. Commit.
        let manifest = Manifest {
            id: j.id.clone(),
            author: self.me.clone(),
            author_name: self.my_name.clone(),
            message: j.message.clone(),
            created_time: j.created_time.clone(),
            files: j
                .items
                .iter()
                .filter_map(|i| Some((i.file_id.clone()?, i.uploaded_rev.clone()?)))
                .chain(j.moves.iter().filter_map(|m| Some((m.file_id.clone(), m.rev.clone()?))))
                .collect::<BTreeMap<_, _>>(),
        };
        if manifest.files.is_empty() {
            // Everything was left out (unlocked pushes): nothing to commit.
            self.index.delete_journal(&j.id)?;
            return Ok(PushOutcome { changeset: j.id.clone(), files: 0, skipped: j.skipped.clone() });
        }
        if !j.committed {
            self.write_manifest(&manifest).await?;
            j.committed = true;
            self.save_journal(j)?;
        }
        self.index.insert_changeset(&manifest)?;

        // 4. Finalize each file.
        for item in &j.items {
            let (Some(id), Some(rev)) = (&item.file_id, &item.uploaded_rev) else { continue };
            self.drive.keep_revision_forever(id, rev).await?;
            let patch = if j.unlocked {
                MetadataPatch::default().unset(props::PENDING_CHANGESET)
            } else if j.keep_locked {
                MetadataPatch::default()
                    .unset(props::PENDING_CHANGESET)
                    .read_only(true, Some(format!("Checked out by {} in Permafrost", self.me)))
            } else {
                Self::unlock_patch().unset(props::PENDING_CHANGESET)
            };
            let f = self.drive.update_metadata(id, patch).await?;
            self.index.apply_remote(std::slice::from_ref(&f), &[])?;

            let abs = self.abs(&item.rel_path);
            let fp = item.pre_fp.or_else(|| localfs::fingerprint(&abs));
            self.index.set_local(&LocalState {
                file_id: id.clone(),
                rel_path: item.rel_path.to_string_lossy().to_string(),
                synced_rev: Some(rev.clone()),
                synced_md5: f.md5.clone(),
                synced_size: fp.map(|x| x.0),
                synced_mtime: fp.map(|x| x.1),
            })?;
            if abs.exists() && !j.unlocked {
                localfs::set_read_only(&abs, !j.keep_locked)?;
            }
        }
        for m in &j.moves {
            if j.items.iter().any(|i| i.file_id.as_deref() == Some(m.file_id.as_str())) {
                continue; // also uploaded: finalized above
            }
            let patch = if m.is_folder {
                MetadataPatch::default().unset(props::PENDING_CHANGESET)
            } else {
                if let Some(rev) = &m.rev {
                    self.drive.keep_revision_forever(&m.file_id, rev).await?;
                }
                if j.keep_locked || j.unlocked {
                    MetadataPatch::default().unset(props::PENDING_CHANGESET)
                } else {
                    Self::unlock_patch().unset(props::PENDING_CHANGESET)
                }
            };
            let f = self.drive.update_metadata(&m.file_id, patch).await?;
            self.index.apply_remote(std::slice::from_ref(&f), &[])?;
            if !m.is_folder && !j.keep_locked && !j.unlocked {
                let abs = self.abs(&m.new_rel);
                if abs.exists() {
                    localfs::set_read_only(&abs, true)?;
                }
            }
        }
        self.index.delete_journal(&j.id)?;
        Ok(PushOutcome {
            changeset: j.id.clone(),
            files: j.items.len() + j.moves.iter().filter(|m| !m.is_folder).count(),
            skipped: j.skipped.clone(),
        })
    }

    async fn upload_item(&self, changeset: &str, unlocked: bool, item: &JournalItem) -> Result<RemoteFile> {
        let src = self.abs(&item.rel_path);
        match &item.file_id {
            Some(id) => {
                let mut patch = MetadataPatch::default().set(props::PENDING_CHANGESET, changeset);
                if !unlocked {
                    // Lift the content restriction just for the upload; the lock
                    // properties stay, so other clients still treat it as checked out.
                    patch = patch.read_only(false, None);
                }
                self.drive.update_metadata(id, patch).await?;
                self.drive.upload_content(id, &src).await
            }
            None => {
                let parent = self.ensure_parent_folder(&item.rel_path).await?;
                let name = item.rel_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let mut props = BTreeMap::from([(props::PENDING_CHANGESET.to_string(), changeset.to_string())]);
                if let Some(key) = &item.create_key {
                    props.insert(props::CREATE_KEY.to_string(), key.clone());
                }
                if !unlocked {
                    props.extend([
                        (props::LOCK_OWNER.to_string(), self.me.clone()),
                        (props::LOCK_OWNER_NAME.to_string(), self.my_name.clone()),
                        (props::LOCK_ID.to_string(), ulid::Ulid::new().to_string()),
                        (props::LOCKED_AT.to_string(), chrono::Utc::now().to_rfc3339()),
                        (props::LOCK_MACHINE.to_string(), self.machine.clone()),
                    ]);
                }
                self.drive.create_file(&parent, &name, &src, props).await
            }
        }
    }

    /// Create any Drive folders missing on the way to a new file.
    pub(super) async fn ensure_parent_folder(&self, rel: &std::path::Path) -> Result<String> {
        let mut parent_id = self.root_id.clone();
        let mut so_far = PathBuf::new();
        let Some(dir) = rel.parent() else { return Ok(parent_id) };
        for comp in dir.components() {
            so_far.push(comp);
            let map = self.paths()?;
            parent_id = match map.id_for(&so_far) {
                Some(id) => id.to_string(),
                None => {
                    let name = comp.as_os_str().to_string_lossy().to_string();
                    let f = self.drive.create_folder(&parent_id, &name).await?;
                    self.index.apply_remote(std::slice::from_ref(&f), &[])?;
                    f.id
                }
            };
        }
        Ok(parent_id)
    }

    /// The hidden `.permafrost` folder (or a legacy `.cadsync` one) in the vault folder, created if missing.
    pub(crate) async fn ensure_meta_folder(&self) -> Result<String> {
        if let Some(id) = self.paths()?.meta_folder_id {
            return Ok(id);
        }
        let f = self.drive.create_folder(&self.root_id, META_FOLDER).await?;
        self.index.apply_remote(std::slice::from_ref(&f), &[])?;
        Ok(f.id)
    }

    async fn write_manifest(&self, m: &Manifest) -> Result<()> {
        let meta = self.ensure_meta_folder().await?;
        let changesets = match self.index.children(&meta)?.into_iter().find(|f| f.is_folder() && f.name == "changesets") {
            Some(f) => f.id,
            None => {
                let f = self.drive.create_folder(&meta, "changesets").await?;
                self.index.apply_remote(std::slice::from_ref(&f), &[])?;
                f.id
            }
        };
        std::fs::create_dir_all(self.scratch_dir())?;
        let tmp = self.scratch_dir().join(format!("{}.json", m.id));
        std::fs::write(&tmp, serde_json::to_vec_pretty(m)?)?;
        let props = BTreeMap::from([(MANIFEST_PROP.to_string(), m.id.clone())]);
        let f = self.drive.create_file(&changesets, &format!("{}.json", m.id), &tmp, props).await?;
        let _ = std::fs::remove_file(&tmp);
        self.index.apply_remote(std::slice::from_ref(&f), &[])?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "push_tests.rs"]
mod tests;
