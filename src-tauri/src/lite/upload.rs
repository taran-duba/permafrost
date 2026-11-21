//! Upload Changes: applying the ticked upload rows to Drive.
//!
//! Uploads go through the same journaled push as Permafrost (pending tags,
//! one manifest, revisions kept forever), so a crash mid-upload resumes and
//! Permafrost users see a normal push in History. There are no locks, so the
//! push re-checks each file just before uploading it: a file someone changed
//! on Drive since this computer last downloaded it is left out, unless the
//! user explicitly chose to overwrite it. A file checked out in Permafrost is
//! never overwritten, overwrite or not.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;

use super::plan::{ChangeItem, Kind};
use crate::drive::{props, DriveApi};
use crate::error::{Error, Result};
use crate::sync::push::{Journal, JournalItem, SkippedFile};
use crate::sync::Vault;

pub const DEFAULT_MESSAGE: &str = "Uploaded from Permafrost Lite";

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadReport {
    /// The push, if anything was uploaded.
    pub changeset: Option<String>,
    pub uploaded: usize,
    pub deleted: usize,
    pub skipped: Vec<SkippedFile>,
}

fn skip(item: &ChangeItem, reason: impl Into<String>) -> SkippedFile {
    SkippedFile { rel_path: PathBuf::from(&item.path), reason: reason.into() }
}

impl<D: DriveApi> Vault<D> {
    /// Apply the upload rows with these keys. `overwrite` lists the Mismatch
    /// rows the user chose to upload over the Drive copy.
    pub async fn lite_upload(&self, keys: &[String], overwrite: &[String], message: &str) -> Result<UploadReport> {
        let plan = self.lite_plan()?;
        self.lite_apply_bookkeeping(&plan)?;
        let overwrite: HashSet<&str> = overwrite.iter().map(String::as_str).collect();
        let mut report = UploadReport::default();
        let mut items = Vec::new();
        let mut deletes = Vec::new();

        for key in keys {
            let Some(row) = plan.upload.iter().find(|i| &i.key == key) else {
                report.skipped.push(SkippedFile { rel_path: PathBuf::from(key), reason: "no longer needs uploading".into() });
                continue;
            };
            if let Some(why) = &row.blocked {
                report.skipped.push(skip(row, why.clone()));
                continue;
            }
            let forced = overwrite.contains(key.as_str());
            if row.kind == Kind::Mismatch && !forced {
                report.skipped.push(skip(row, "changed on Drive too; download it first, or choose Overwrite"));
                continue;
            }
            match self.upload_item_for(row, forced) {
                Ok(Some(item)) => items.push(item),
                Ok(None) => deletes.push((row, forced)),
                Err(e) => report.skipped.push(skip(row, e.to_string())),
            }
        }

        if !items.is_empty() {
            let message = if message.trim().is_empty() { DEFAULT_MESSAGE } else { message.trim() };
            let out = self.run_push(Journal::new(message.to_string(), items, true)).await?;
            report.uploaded = out.files;
            report.changeset = (out.files > 0).then_some(out.changeset);
            report.skipped.extend(out.skipped);
        }
        for (row, forced) in deletes {
            match self.lite_trash(row, forced).await {
                Ok(()) => report.deleted += 1,
                Err(e) => report.skipped.push(skip(row, e.to_string())),
            }
        }
        Ok(report)
    }

    /// The push item for an upload row, or `None` for a row that deletes the Drive file.
    fn upload_item_for(&self, row: &ChangeItem, forced: bool) -> Result<Option<JournalItem>> {
        let rel = PathBuf::from(&row.path);
        let local_exists = self.abs(&rel).is_file();
        let remote = row.file_id.as_deref().map(|id| self.index.file(id)).transpose()?.flatten();
        match (row.kind, remote) {
            (Kind::Delete, _) => Ok(None),
            (Kind::Mismatch, Some(_)) if !local_exists => Ok(None), // deleted here, changed on Drive
            (_, Some(f)) => {
                self.ensure_can_edit(&f)?;
                let mut item = JournalItem::new(Some(f.id.clone()), rel);
                item.base_rev = self.index.local(&f.id)?.and_then(|l| l.synced_rev);
                item.overwrite = forced;
                Ok(Some(item))
            }
            (_, None) => {
                // New, or deleted on Drive but changed here: added as a new file.
                if self.paths()?.id_for(&rel).is_some() {
                    return Err(Error::Conflict("a file with this name is already on Drive".into()));
                }
                self.ensure_can_add(&rel)?;
                Ok(Some(JournalItem::new(None, rel)))
            }
        }
    }

    /// Move a Drive file to the trash (recoverable for 30 days), unless someone
    /// changed, locked or is uploading it since this computer last synced it.
    async fn lite_trash(&self, row: &ChangeItem, forced: bool) -> Result<()> {
        let id = row.file_id.as_deref().ok_or_else(|| Error::NotFound(row.path.clone()))?;
        let f = self.drive.get_file(id).await?;
        self.index.apply_remote(std::slice::from_ref(&f), &[])?;
        if f.trashed {
            self.index.remove_local(id)?;
            return Ok(());
        }
        self.ensure_can_edit(&f)?;
        if let Some(l) = f.lock() {
            return Err(Error::Locked { name: f.name, by: l.display_owner().to_string() });
        }
        if f.read_only {
            return Err(Error::Conflict("read-only on Drive".into()));
        }
        if f.prop(props::PENDING_CHANGESET).is_some_and(|cs| self.pending_blocks(cs).unwrap_or(true)) {
            return Err(Error::Conflict("someone is uploading it right now".into()));
        }
        let base = self.index.local(id)?.and_then(|l| l.synced_rev);
        if !forced && f.head_revision_id != base {
            return Err(Error::Conflict("changed on Drive since you last downloaded it; download it first".into()));
        }
        self.drive.trash_file(id).await?;
        tracing::info!(file = %f.name, by = %self.me, "moved to Drive's trash from Permafrost Lite");
        self.index.apply_remote(&[], &[id.to_string()])?;
        self.index.remove_local(id)?;
        Ok(())
    }
}
