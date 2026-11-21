//! Download Changes: applying the ticked download rows to this computer.
//!
//! Every file is downloaded to staging and checked against Drive's md5 first;
//! only when all of them arrived intact are they swapped in. A dropped
//! connection halfway leaves every local file as it was. Anything a download
//! would overwrite or delete that has local edits is copied to the app's
//! backups folder first, outside the project so SolidWorks never sees it.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::plan::{ChangeItem, Kind};
use crate::drive::DriveApi;
use crate::error::{Error, Result};
use crate::index::LocalState;
use crate::sync::{localfs, Vault};

const DOWNLOAD_ATTEMPTS: usize = 3;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadReport {
    pub downloaded: usize,
    pub moved: usize,
    pub deleted: usize,
    /// Local copies saved before being overwritten or deleted.
    pub backups: Vec<PathBuf>,
    /// Rows not applied, with the reason.
    pub skipped: Vec<String>,
}

struct Staged<'a> {
    item: &'a ChangeItem,
    tmp: PathBuf,
    rev: String,
    md5: Option<String>,
}

impl<D: DriveApi> Vault<D> {
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    /// Copy a local file into the backups folder, keeping its project path.
    fn backup(&self, rel: &Path, stamp: &str) -> Result<PathBuf> {
        let target = self.backups_dir().join(stamp).join(rel);
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::copy(self.abs(rel), &target)?;
        Ok(target)
    }

    async fn download_with_retry(&self, id: &str, rev: &str, md5: Option<&str>, tmp: &Path) -> Result<()> {
        let mut last = None;
        for attempt in 1..=DOWNLOAD_ATTEMPTS {
            let _ = std::fs::remove_file(tmp);
            match self.drive.download_revision(id, rev, tmp).await {
                Ok(()) => match md5 {
                    Some(want) if localfs::md5_file(tmp)? != want => {
                        last = Some(Error::Conflict("the download didn't match Drive's copy".into()));
                    }
                    _ => return Ok(()),
                },
                Err(e) => last = Some(e),
            }
            tracing::warn!(file = id, attempt, "download failed; retrying");
        }
        Err(last.unwrap_or_else(|| Error::Network("download failed".into())))
    }

    /// Apply the download rows with these keys. Rows are looked up in a fresh
    /// plan, so a row that no longer applies is skipped rather than forced.
    pub async fn lite_download(&self, keys: &[String]) -> Result<DownloadReport> {
        let plan = self.lite_plan()?;
        self.lite_apply_bookkeeping(&plan)?;
        let mut report = DownloadReport::default();
        let picked: Vec<&ChangeItem> = keys
            .iter()
            .filter_map(|k| {
                let found = plan.download.iter().find(|i| &i.key == k);
                if found.is_none() {
                    report.skipped.push(format!("{k}: no longer needs downloading"));
                }
                found
            })
            .collect();
        let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S").to_string();
        let stage = self.staging_dir().join(ulid::Ulid::new().to_string());
        std::fs::create_dir_all(&stage)?;
        let result = self.download_staged(&picked, &stage, &stamp, &mut report).await;
        let _ = std::fs::remove_dir_all(&stage);
        result.map(|()| report)
    }

    async fn download_staged(&self, picked: &[&ChangeItem], stage: &Path, stamp: &str, report: &mut DownloadReport) -> Result<()> {
        // 1. Download everything that has content to staging, verified.
        let mut staged = Vec::new();
        for (n, item) in picked.iter().enumerate() {
            let needs_content = matches!(item.kind, Kind::New | Kind::Revision | Kind::Mismatch);
            let Some(id) = item.file_id.as_deref().filter(|_| needs_content) else { continue };
            let Some(f) = self.index.file(id)? else { continue }; // a Mismatch with Drive deleted
            let Some(rev) = f.head_revision_id.clone() else { continue };
            let tmp = stage.join(n.to_string());
            self.download_with_retry(id, &rev, f.md5.as_deref(), &tmp).await?;
            staged.push(Staged { item, tmp, rev, md5: f.md5.clone() });
        }

        // 2. Swap in downloads, then moves, then deletes.
        for s in &staged {
            let dest_rel = PathBuf::from(&s.item.path);
            let dest = self.abs(&dest_rel);
            if localfs::is_open_in_cad(&dest) {
                report.skipped.push(format!("{}: close it in SolidWorks first", s.item.path));
                continue;
            }
            let id = s.item.file_id.as_deref().unwrap_or_default();
            if dest.exists() && self.has_local_edits(id, &dest_rel)? {
                report.backups.push(self.backup(&dest_rel, stamp)?);
            }
            localfs::replace(&s.tmp, &dest)?;
            localfs::set_read_only(&dest, false)?;
            if let Some(from) = s.item.from_path.as_deref().map(PathBuf::from) {
                let old = self.abs(&from);
                if old.exists() && old != dest && !localfs::is_open_in_cad(&old) {
                    localfs::move_to_trash(&old, &self.trash_dir())?;
                }
            }
            let fp = localfs::fingerprint(&dest);
            self.index.set_local(&LocalState {
                file_id: id.to_string(),
                rel_path: dest_rel.to_string_lossy().to_string(),
                synced_rev: Some(s.rev.clone()),
                synced_md5: s.md5.clone(),
                synced_size: fp.map(|x| x.0),
                synced_mtime: fp.map(|x| x.1),
            })?;
            report.downloaded += 1;
        }
        for item in picked.iter().filter(|i| i.kind == Kind::Move) {
            self.apply_move(item, report)?;
        }
        for item in picked.iter().filter(|i| matches!(i.kind, Kind::Delete | Kind::Mismatch)) {
            let gone_on_drive = item.file_id.as_deref().map(|id| self.index.file(id)).transpose()?.flatten().is_none();
            let id = item.key.trim_start_matches("d:");
            if item.kind == Kind::Mismatch && !gone_on_drive {
                continue; // a content mismatch: handled with the downloads
            }
            let rel = PathBuf::from(&item.path);
            let p = self.abs(&rel);
            if localfs::is_open_in_cad(&p) {
                report.skipped.push(format!("{}: close it in SolidWorks first", item.path));
                continue;
            }
            if p.exists() {
                if item.kind == Kind::Mismatch {
                    report.backups.push(self.backup(&rel, stamp)?);
                }
                localfs::move_to_trash(&p, &self.trash_dir())?;
            }
            self.index.remove_local(id)?;
            report.deleted += 1;
        }
        Ok(())
    }

    fn apply_move(&self, item: &ChangeItem, report: &mut DownloadReport) -> Result<()> {
        let (Some(id), Some(from)) = (item.file_id.as_deref(), item.from_path.as_deref()) else { return Ok(()) };
        let (from, to) = (self.abs(Path::new(from)), self.abs(Path::new(&item.path)));
        if localfs::is_open_in_cad(&from) {
            report.skipped.push(format!("{}: close it in SolidWorks first", item.path));
            return Ok(());
        }
        if to.exists() {
            report.skipped.push(format!("{}: a file with that name is already on this computer", item.path));
            return Ok(());
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(&from, &to)?;
        if let Some(mut l) = self.index.local(id)? {
            l.rel_path = PathBuf::from(&item.path).to_string_lossy().to_string();
            self.index.set_local(&l)?;
        }
        report.moved += 1;
        Ok(())
    }

    /// Does the local file have edits that a download would lose?
    fn has_local_edits(&self, file_id: &str, rel: &Path) -> Result<bool> {
        match self.index.local(file_id)? {
            Some(l) if Path::new(&l.rel_path) == rel => self.lite_modified(&l),
            // Never synced, or a different file sits at this path: keep a copy to be safe.
            _ => Ok(true),
        }
    }
}
