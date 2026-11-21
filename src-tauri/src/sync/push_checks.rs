//! Checks made during a push, just before each upload: is an unlocked
//! (Permafrost Lite) upload still safe, and did an interrupted attempt already
//! create this new file?

use super::push::JournalItem;
use super::Vault;
use crate::drive::{props, DriveApi, RemoteFile};
use crate::error::{Error, Result};

impl<D: DriveApi> Vault<D> {
    /// Unlocked pushes: why this file can't be uploaded right now, if it can't.
    /// Checked just before each upload, since nothing stops a teammate from
    /// changing or locking the file between the verify step and the upload.
    pub(super) async fn unlocked_blocker(&self, item: &JournalItem) -> Result<Option<String>> {
        let Some(id) = &item.file_id else {
            let taken = self.paths()?.id_for(&item.rel_path).is_some();
            return Ok(taken.then(|| "someone added a file with this name on Drive".to_string()));
        };
        let f = match self.drive.get_file(id).await {
            Ok(f) => f,
            Err(Error::NotFound(_)) | Err(Error::Drive { status: 404, .. }) => return Ok(Some("deleted on Drive".into())),
            Err(e) => return Err(e),
        };
        self.index.apply_remote(std::slice::from_ref(&f), &[])?;
        if f.trashed {
            return Ok(Some("deleted on Drive".into()));
        }
        if let Some(l) = f.lock() {
            return Ok(Some(format!("checked out in Permafrost by {}", l.display_owner())));
        }
        if f.read_only {
            return Ok(Some("read-only on Drive".into()));
        }
        if !item.overwrite && f.head_revision_id != item.base_rev {
            let by = f.last_modifier.as_deref().unwrap_or("someone");
            return Ok(Some(format!("changed on Drive by {by}; download it first")));
        }
        Ok(None)
    }

    /// A new file an earlier, interrupted attempt already created, found by its create key.
    pub(super) async fn find_created(&self, item: &JournalItem, retry: bool) -> Result<Option<RemoteFile>> {
        let (None, Some(key), true) = (&item.file_id, &item.create_key, retry) else { return Ok(None) };
        let parent = self.ensure_parent_folder(&item.rel_path).await?;
        let mut token = None;
        loop {
            let page = self.drive.list_children(self.scope.as_deref(), std::slice::from_ref(&parent), token.as_deref()).await?;
            if let Some(f) = page.files.into_iter().find(|f| f.prop(props::CREATE_KEY) == Some(key.as_str())) {
                tracing::info!(file = %item.rel_path.display(), "found the file an interrupted push already created");
                return Ok(Some(f));
            }
            match page.next_page_token {
                Some(t) => token = Some(t),
                None => return Ok(None),
            }
        }
    }
}
