//! Permafrost Lite's smoke-test checks, run by the built app against the demo
//! drive: the first download, an upload a Permafrost teammate receives as a
//! normal push, a file checked out in Permafrost is never overwritten, and a
//! teammate's change arrives as a download.

use std::time::Duration;

use super::AppState;
use crate::drive::Backend;
use crate::index::Index;
use crate::lite::plan::Kind;
use crate::sync::{localfs, Vault};

type Check = Result<(), String>;

fn ensure(ok: bool, what: &str) -> Check {
    if ok { Ok(()) } else { Err(format!("Lite check failed: {what}")) }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Download everything the fresh demo offers. Returns how many files came down.
pub async fn first_download(state: &AppState) -> Result<usize, String> {
    let v = state.vault().await.map_err(err)?;
    let plan = v.lite_plan().map_err(err)?;
    ensure(plan.download.len() >= 11 && plan.download.iter().all(|i| i.kind == Kind::New), "the demo should offer its files as new downloads")?;
    ensure(plan.upload.is_empty(), "nothing should be ready to upload yet")?;
    let keys: Vec<String> = plan.download.iter().map(|i| i.key.clone()).collect();
    let r = v.lite_download(&keys).await.map_err(|e| format!("download: {e}"))?;
    ensure(r.skipped.is_empty(), &format!("downloads skipped: {:?}", r.skipped))?;
    for l in v.index.all_local().map_err(err)? {
        let p = v.abs(l.rel_path.as_ref());
        ensure(p.is_file(), &format!("{} is missing", l.rel_path))?;
        ensure(!localfs::is_read_only(&p), &format!("{} is read-only; Lite files must stay writable", l.rel_path))?;
    }
    ensure(v.lite_plan().map_err(err)?.download.is_empty(), "downloads still listed after downloading")?;
    Ok(r.downloaded)
}

pub async fn features(state: &AppState) -> Result<Vec<String>, String> {
    let v = state.vault().await.map_err(err)?;
    let fake = match &v.drive {
        Backend::Fake(d) => d.clone(),
        Backend::Http(_) => return Err("Lite checks need the demo drive".into()),
    };
    let rel_of = |name: &str| -> Result<(String, std::path::PathBuf), String> {
        let map = v.paths().map_err(err)?;
        let f = v.index.all_files().map_err(err)?.into_iter().find(|f| f.name == name).ok_or(format!("demo has no {name}"))?;
        let rel = map.path(&f.id).ok_or(format!("{name} has no path"))?.to_path_buf();
        Ok((f.id, rel))
    };
    let mut passed = Vec::new();

    // A teammate using full Permafrost, on their own PC.
    let dir = std::env::temp_dir().join("permafrost-lite-smoke-jordan");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("vault")).map_err(err)?;
    let jordan = Vault {
        drive: Backend::Fake(fake.as_user("jordan@demo.team")),
        index: Index::open(&dir.join("index.sqlite")).map_err(err)?,
        root_id: v.root_id.clone(),
        scope: None,
        root: dir.join("vault"),
        data_dir: dir.clone(),
        me: "jordan@demo.team".into(),
        my_name: "Jordan Lee".into(),
        machine: "SHOP-PC-2".into(),
        lock_verify_delay: Duration::from_millis(0),
    };
    jordan.sync_down().await.map_err(|e| format!("teammate sync: {e}"))?;

    // 1. An upload is a normal push for Permafrost users.
    let (tube, tube_rel) = rel_of("Arm Tube.SLDPRT")?;
    std::fs::write(v.abs(&tube_rel), b"arm tube, edited in Lite").map_err(err)?;
    let plan = v.lite_plan().map_err(err)?;
    let row = plan.upload.iter().find(|i| i.file_id.as_deref() == Some(tube.as_str())).ok_or("the edit isn't listed to upload")?;
    ensure(row.kind == Kind::Revision, "an edit should be a revision")?;
    let up = v.lite_upload(&[row.key.clone()], &[], "smoke test").await.map_err(|e| format!("upload: {e}"))?;
    let cs = up.changeset.ok_or("nothing was uploaded")?;
    jordan.sync_down().await.map_err(|e| format!("teammate sync: {e}"))?;
    ensure(jordan.index.changeset(&cs).map_err(err)?.is_some(), "the upload isn't in the teammate's History")?;
    ensure(std::fs::read(jordan.abs(&tube_rel)).map_err(err)? == b"arm tube, edited in Lite", "the teammate didn't get the upload")?;
    passed.push("upload: a Permafrost teammate gets it as a normal push".to_string());

    // 2. The demo's checked-out motor is never overwritten, even with Overwrite.
    let (motor, motor_rel) = rel_of("Kraken X60.SLDPRT")?;
    std::fs::write(v.abs(&motor_rel), b"motor, edited in Lite").map_err(err)?;
    let plan = v.lite_plan().map_err(err)?;
    let row = plan.upload.iter().find(|i| i.file_id.as_deref() == Some(motor.as_str())).ok_or("the motor edit isn't listed")?;
    ensure(row.blocked.is_some(), "a file checked out in Permafrost should be blocked")?;
    let up = v.lite_upload(&[row.key.clone()], &[row.key.clone()], "").await.map_err(err)?;
    ensure(up.uploaded == 0 && fake.content(&motor).as_deref() == Some(b"motor".as_slice()), "a checked-out file was overwritten")?;
    passed.push("locks: never overwrites a file checked out in Permafrost".to_string());

    // 3. A teammate's change shows up as a download.
    let (plate, plate_rel) = rel_of("Pivot Plate.SLDPRT")?;
    fake.external_edit(&plate, b"pivot plate v2");
    v.refresh().await.map_err(err)?;
    let plan = v.lite_plan().map_err(err)?;
    let row = plan.download.iter().find(|i| i.file_id.as_deref() == Some(plate.as_str())).ok_or("the teammate's change isn't listed")?;
    ensure(row.kind == Kind::Revision, "a teammate's change should be a revision")?;
    v.lite_download(&[row.key.clone()]).await.map_err(err)?;
    ensure(std::fs::read(v.abs(&plate_rel)).map_err(err)? == b"pivot plate v2", "the download didn't arrive")?;
    passed.push("download: a teammate's change arrives".to_string());

    let _ = std::fs::remove_dir_all(&dir);
    Ok(passed)
}
