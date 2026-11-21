//! Tests for atomic check-ins (`push.rs`).

use super::*;
use crate::drive::fake::FakeDrive;
use crate::sync::pull::Deferred;
use crate::sync::testutil::vault;

/// Robot.SLDASM + Gear.SLDPRT on Drive; Alex and Blair both synced.
async fn team() -> (FakeDrive, String, String, crate::sync::testutil::Fixture, crate::sync::testutil::Fixture) {
    let drive = FakeDrive::new("seed@team.org");
    let root = drive.drive_id.clone();
    let gear = drive.seed_file(&root, "Gear.SLDPRT", b"gear v1");
    let robot = drive.seed_file(&root, "Robot.SLDASM", b"robot v1");
    let a = vault(&drive, "alex@team.org", "pc-a");
    let b = vault(&drive, "blair@team.org", "pc-b");
    for v in [&a.vault, &b.vault] {
        v.sync_down().await.unwrap();
    }
    (drive, gear, robot, a, b)
}

fn edit(v: &Vault<FakeDrive>, rel: &str, bytes: &[u8]) {
    std::fs::write(v.root.join(rel), bytes).unwrap();
}

#[tokio::test]
async fn assembly_and_parts_arrive_together() {
    let (drive, gear, robot, a, b) = team().await;
    let av = &a.vault;
    av.check_out(&gear).await.unwrap();
    av.check_out(&robot).await.unwrap();
    edit(av, "Gear.SLDPRT", b"gear v2");
    edit(av, "Robot.SLDASM", b"robot v2");
    std::fs::write(av.root.join("Spacer.SLDPRT"), b"spacer v1").unwrap();

    let out = av
        .push(PushRequest {
            files: vec![robot.clone(), gear.clone()],
            new_files: vec!["Spacer.SLDPRT".into()],
            message: "new gear ratio".into(),
            keep_locked: false,
            moves: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.files, 3);
    assert!(av.index.changeset(&out.changeset).unwrap().is_some());
    let gf = drive.get_file(&gear).await.unwrap();
    assert!(gf.lock().is_none() && !gf.read_only && gf.prop(props::PENDING_CHANGESET).is_none());
    assert!(drive.is_kept_forever(&gear, gf.head_revision_id.as_deref().unwrap()));
    assert!(localfs::is_read_only(&av.root.join("Gear.SLDPRT")), "released → read-only again");

    let r = b.vault.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 3, "{r:?}");
    assert_eq!(std::fs::read(b.vault.root.join("Robot.SLDASM")).unwrap(), b"robot v2");
    assert_eq!(std::fs::read(b.vault.root.join("Spacer.SLDPRT")).unwrap(), b"spacer v1");
}

#[tokio::test]
async fn half_finished_push_is_invisible_then_resumes() {
    let (drive, gear, robot, a, b) = team().await;
    let av = &a.vault;
    av.check_out(&gear).await.unwrap();
    av.check_out(&robot).await.unwrap();
    edit(av, "Gear.SLDPRT", b"gear v2");
    edit(av, "Robot.SLDASM", b"robot v2");

    drive.crash_after_uploads(1); // gear uploads, then the "app crashes"
    let req = PushRequest { files: vec![gear.clone(), robot.clone()], message: "m".into(), ..Default::default() };
    assert!(av.push(req).await.is_err());
    assert_eq!(drive.content(&gear).unwrap(), b"gear v2", "gear really is on Drive");

    // Blair sees neither file yet: the gear is tagged with an uncommitted changeset.
    let r = b.vault.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 0);
    assert!(r.deferred.iter().any(|d| matches!(d, Deferred::Pending { file_id, .. } if *file_id == gear)));
    assert_eq!(std::fs::read(b.vault.root.join("Gear.SLDPRT")).unwrap(), b"gear v1");

    drive.clear_faults();
    let done = av.resume_pushes().await.unwrap();
    assert_eq!(done.len(), 1);
    assert!(av.index.journal().unwrap().is_empty());

    let r = b.vault.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 2);
    assert_eq!(std::fs::read(b.vault.root.join("Gear.SLDPRT")).unwrap(), b"gear v2");
    assert_eq!(std::fs::read(b.vault.root.join("Robot.SLDASM")).unwrap(), b"robot v2");
}

#[tokio::test]
async fn refuses_files_not_checked_out_or_changed_underneath() {
    let (drive, gear, robot, a, b) = team().await;
    let av = &a.vault;
    let req = |files: Vec<String>| PushRequest { files, message: "m".into(), ..Default::default() };
    assert!(matches!(av.push(req(vec![gear.clone()])).await, Err(Error::NotCheckedOut(_))));

    b.vault.check_out(&robot).await.unwrap();
    assert!(matches!(av.push(req(vec![robot.clone()])).await, Err(Error::Locked { .. })));

    // Lock broken and file replaced behind Alex's back: conflict, nothing uploaded.
    av.check_out(&gear).await.unwrap();
    edit(av, "Gear.SLDPRT", b"alex's gear");
    b.vault.break_lock(&gear).await.unwrap();
    drive.external_edit(&gear, b"someone else's gear");
    let relock = crate::drive::LockInfo {
        owner: av.me.clone(),
        owner_name: av.my_name.clone(),
        lock_id: "x".into(),
        locked_at: String::new(),
        machine: av.machine.clone(),
    };
    av.drive.update_metadata(&gear, av.lock_patch(&relock)).await.unwrap();
    assert!(matches!(av.push(req(vec![gear.clone()])).await, Err(Error::Conflict(_))));
    assert_eq!(std::fs::read(av.root.join("Gear.SLDPRT")).unwrap(), b"alex's gear", "local work kept");
}

/// The vault is a folder deep inside a bigger drive: paths start at that
/// folder, new files and check-in records land inside it, and the rest of
/// the drive is never touched.
#[tokio::test]
async fn works_when_the_vault_is_a_subfolder() {
    let drive = FakeDrive::new("seed@team.org");
    let top = drive.drive_id.clone();
    let cache = drive.seed_folder(&top, "CacheCAD");
    let robot = drive.seed_folder(&cache, "Robot");
    let gear = drive.seed_file(&robot, "Gear.SLDPRT", b"gear v1");
    drive.seed_file(&top, "Unrelated.txt", b"keep out");

    let mut a = vault(&drive, "alex@team.org", "pc-a");
    let mut b = vault(&drive, "blair@team.org", "pc-b");
    for v in [&mut a.vault, &mut b.vault] {
        v.root_id = cache.clone();
        v.sync_down().await.unwrap();
    }
    let (av, bv) = (&a.vault, &b.vault);
    assert_eq!(std::fs::read(av.root.join("Robot/Gear.SLDPRT")).unwrap(), b"gear v1");
    assert!(!av.root.join("Unrelated.txt").exists(), "files outside the folder aren't synced");

    av.check_out(&gear).await.unwrap();
    edit(av, "Robot/Gear.SLDPRT", b"gear v2");
    std::fs::write(av.root.join("Robot/Spacer.SLDPRT"), b"new").unwrap();
    let out = av
        .push(PushRequest {
            files: vec![gear.clone()],
            new_files: vec!["Robot/Spacer.SLDPRT".into()],
            message: "m".into(),
            keep_locked: false,
            moves: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.files, 2);

    // Check-in records went inside the chosen folder, not the drive root.
    let meta: Vec<_> = av.index.all_files().unwrap().into_iter().filter(|f| f.name == ".permafrost").collect();
    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].parent.as_deref(), Some(cache.as_str()), ".permafrost lives in the vault folder");

    let r = bv.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 2);
    assert_eq!(std::fs::read(bv.root.join("Robot/Gear.SLDPRT")).unwrap(), b"gear v2");
    assert_eq!(std::fs::read(bv.root.join("Robot/Spacer.SLDPRT")).unwrap(), b"new");
}

#[tokio::test]
async fn transient_upload_errors_leave_a_resumable_journal() {
    let (drive, gear, _robot, a, _b) = team().await;
    let av = &a.vault;
    av.check_out(&gear).await.unwrap();
    edit(av, "Gear.SLDPRT", b"gear v2");
    drive.fail_next_uploads(1);
    let req = PushRequest { files: vec![gear.clone()], message: "m".into(), keep_locked: true, ..Default::default() };
    assert!(av.push(req).await.is_err());
    assert_eq!(av.index.journal().unwrap().len(), 1);
    av.resume_pushes().await.unwrap();
    let f = drive.get_file(&gear).await.unwrap();
    assert!(av.i_hold_lock(&f) && f.read_only, "kept checked out");
    assert!(!localfs::is_read_only(&av.root.join("Gear.SLDPRT")));
}
