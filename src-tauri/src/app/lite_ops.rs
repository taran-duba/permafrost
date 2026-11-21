//! Tauri commands only Permafrost Lite has: the Upload/Download lists, the
//! root folder, the default Drive folder for new projects, and New Project.

use std::path::{Component, Path, PathBuf};

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use super::config::{folder_id_from_input, is_valid_id, Project};
use super::AppState;
use crate::drive::DriveApi;
use crate::error::{Error, Result};
use crate::lite::download::DownloadReport;
use crate::lite::plan::LitePlan;
use crate::lite::upload::UploadReport;

fn io_err(e: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(e.to_string()))
}

fn check_keys(keys: &[String]) -> Result<()> {
    if keys.len() > 100_000 || keys.iter().any(|k| k.len() > 2048) {
        return Err(Error::Config("too many files picked".into()));
    }
    Ok(())
}

/// What's ready to upload and download in the open project.
#[tauri::command]
pub async fn lite_changes(state: State<'_, AppState>) -> Result<LitePlan> {
    let v = state.vault().await?;
    // Hashing changed files is disk work; keep it off the async workers.
    tauri::async_runtime::spawn_blocking(move || {
        let plan = v.lite_plan()?;
        v.lite_apply_bookkeeping(&plan)?;
        Ok(plan)
    })
    .await
    .map_err(io_err)?
}

#[tauri::command]
pub async fn lite_download(app: AppHandle, state: State<'_, AppState>, keys: Vec<String>) -> Result<DownloadReport> {
    check_keys(&keys)?;
    let v = state.vault().await?;
    let guard = state.sync_lock.lock().await;
    let r = v.lite_download(&keys).await;
    drop(guard);
    match &r {
        Ok(rep) => tracing::info!(downloaded = rep.downloaded, moved = rep.moved, deleted = rep.deleted, skipped = rep.skipped.len(), "downloaded changes"),
        Err(e) => tracing::warn!(%e, "download failed"),
    }
    let out = state.check_expired(&app, r).await?;
    let _ = app.emit("vault-changed", ());
    Ok(out)
}

#[tauri::command]
pub async fn lite_upload(
    app: AppHandle,
    state: State<'_, AppState>,
    keys: Vec<String>,
    overwrite: Vec<String>,
    message: String,
) -> Result<UploadReport> {
    check_keys(&keys)?;
    check_keys(&overwrite)?;
    if message.chars().count() > 2000 {
        return Err(Error::Config("the message is too long".into()));
    }
    let v = state.vault().await?;
    let guard = state.sync_lock.lock().await;
    // Catch up with Drive first, so the upload compares against the latest.
    let r = match v.refresh().await {
        Ok(_) => v.lite_upload(&keys, &overwrite, &message).await,
        Err(e) => Err(e),
    };
    drop(guard);
    match &r {
        Ok(rep) => tracing::info!(uploaded = rep.uploaded, deleted = rep.deleted, skipped = rep.skipped.len(), "uploaded changes"),
        Err(e) => tracing::warn!(%e, "upload failed"),
    }
    let out = state.check_expired(&app, r).await?;
    let _ = app.emit("vault-changed", ());
    Ok(out)
}

/// Set the folder project folders go in. Projects already added stay where they are.
#[tauri::command]
pub async fn lite_set_root(app: AppHandle, state: State<'_, AppState>, root: PathBuf) -> Result<()> {
    if !root.is_absolute() {
        return Err(Error::Config("pick a full folder path".into()));
    }
    std::fs::create_dir_all(&root)?;
    let mut cfg = state.config.write().await;
    cfg.lite_root = Some(root);
    cfg.save(&state.config_path)?;
    drop(cfg);
    let _ = app.emit("vault-changed", ());
    Ok(())
}

/// Set the Drive folder that New Project creates project folders in. Returns its name.
#[tauri::command]
pub async fn lite_set_default_parent(app: AppHandle, state: State<'_, AppState>, input: String) -> Result<String> {
    let id = if input.trim() == "root" {
        "root".to_string()
    } else {
        folder_id_from_input(&input).ok_or_else(|| Error::Config("that doesn't look like a Drive folder link".into()))?
    };
    let r = state.http().await?.folder_info(&id).await;
    let info = state.check_expired(&app, r).await?;
    if !info.is_folder {
        return Err(Error::Config("that link isn't a folder".into()));
    }
    let name = if id == "root" { "My Drive".to_string() } else { info.name };
    let mut cfg = state.config.write().await;
    cfg.default_parent = Some(info.id);
    cfg.default_parent_name = Some(name.clone());
    cfg.save(&state.config_path)?;
    Ok(name)
}

/// Open the default Drive folder for new projects on the Drive website.
#[tauri::command]
pub async fn lite_open_default_parent(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let url = match state.config.read().await.default_parent.clone() {
        Some(id) if is_valid_id(&id) => format!("https://drive.google.com/drive/folders/{id}"),
        _ => "https://drive.google.com/drive/my-drive".to_string(),
    };
    app.opener().open_url(url, None::<&str>).map_err(io_err)
}

/// Open the root folder (where project folders go) in Explorer/Finder.
#[tauri::command]
pub async fn lite_reveal_base(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let base = state.config.read().await.base_dir();
    std::fs::create_dir_all(&base)?;
    app.opener().open_path(base.to_string_lossy(), None::<&str>).map_err(io_err)
}

/// Open the open project's backups folder: local copies a download replaced or deleted.
#[tauri::command]
pub async fn lite_reveal_backups(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let dir = state.vault().await?.backups_dir();
    std::fs::create_dir_all(&dir)?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(io_err)
}

/// Where a new project's local folder goes: a folder name inside the root
/// folder, or a full path that must be inside it.
fn local_root_for(base: &Path, local_folder: &str) -> Result<Option<PathBuf>> {
    let local_folder = local_folder.trim();
    if local_folder.is_empty() {
        return Ok(None);
    }
    let p = Path::new(local_folder);
    if p.is_absolute() {
        let inside = p.starts_with(base) && p != base && p.components().all(|c| !matches!(c, Component::ParentDir));
        if !inside {
            return Err(Error::Config(format!("pick a folder inside {}", base.display())));
        }
        return Ok(Some(p.to_path_buf()));
    }
    let mut comps = p.components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(name)), None) => Ok(Some(base.join(crate::names::sanitize(&name.to_string_lossy())))),
        _ => Err(Error::Config("use a plain folder name".into())),
    }
}

/// New Project: link an existing Drive folder (`link`), or create a new one
/// named `new_name` in the default Drive folder; then sync it into
/// `local_folder` inside the root folder.
#[tauri::command]
pub async fn lite_add_project(
    app: AppHandle,
    state: State<'_, AppState>,
    link: Option<String>,
    new_name: Option<String>,
    local_folder: Option<String>,
) -> Result<()> {
    if state.demo.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Config("the demo can't add projects; sign in with Google to add your own".into()));
    }
    let http = state.http().await?;
    let cfg = state.config.read().await.clone();
    let folder_id = match (link.as_deref().map(str::trim).filter(|s| !s.is_empty()), new_name.as_deref().map(str::trim)) {
        (Some(link), _) => folder_id_from_input(link).ok_or_else(|| Error::Config("that doesn't look like a Drive folder link".into()))?,
        (None, Some(name)) if !name.is_empty() && name.chars().count() <= 200 && !name.chars().any(char::is_control) => {
            let parent = cfg
                .default_parent
                .clone()
                .ok_or_else(|| Error::Config("choose a default Drive folder in Settings first".into()))?;
            let r = http.create_folder(&parent, name).await;
            state.check_expired(&app, r).await?.id
        }
        _ => return Err(Error::Config("paste a Drive folder link, or name the new folder".into())),
    };
    let r = http.folder_info(&folder_id).await;
    let info = state.check_expired(&app, r).await?;
    if !info.is_folder {
        return Err(Error::Config("that link isn't a folder".into()));
    }
    let root = local_root_for(&cfg.base_dir(), local_folder.as_deref().unwrap_or(""))?
        .unwrap_or_else(|| cfg.root_for(&info.id, &info.name));
    if cfg.projects.iter().any(|p| p.root == root && p.folder_id != info.id) {
        return Err(Error::Config("another project already uses that folder".into()));
    }
    {
        let mut cfg = state.config.write().await;
        cfg.upsert_project(Project { folder_id: info.id.clone(), name: info.name.clone(), root });
        cfg.save(&state.config_path)?;
    }
    let r = state.open_folder(&info.id).await;
    state.check_expired(&app, r).await?;
    let _ = app.emit("vault-changed", ());
    super::commands::sync_soon(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_folders_stay_inside_the_root_folder() {
        // A full path on this OS: "/home/..." has no drive letter, so Windows doesn't treat it as one.
        let base = if cfg!(windows) { Path::new(r"C:\Users\lee\Documents\Permafrost Lite") } else { Path::new("/home/lee/Documents/Permafrost Lite") };
        assert_eq!(local_root_for(base, "").unwrap(), None);
        assert_eq!(local_root_for(base, "Robot 2026").unwrap(), Some(base.join("Robot 2026")));
        assert_eq!(local_root_for(base, &base.join("Arm").to_string_lossy()).unwrap(), Some(base.join("Arm")));
        assert!(local_root_for(base, "../Elsewhere").is_err());
        assert!(local_root_for(base, "a/b").is_err());
        assert!(local_root_for(base, if cfg!(windows) { r"C:\Windows" } else { "/etc" }).is_err());
        assert!(local_root_for(base, &base.to_string_lossy()).is_err());
    }
}
