//! Permafrost Lite against the fake Drive, with a full Permafrost user ("perm")
//! on the same folder to check the two coexist.

use super::plan::{ChangeItem, Kind, LitePlan};
use crate::drive::fake::FakeDrive;
use crate::drive::{props, DriveApi, MetadataPatch};
use crate::sync::push::PushRequest;
use crate::sync::testutil::{vault, Fixture};
use crate::sync::{localfs, Vault};

type V = Vault<FakeDrive>;

/// Gear.SLDPRT + Robot.SLDASM on Drive; a Lite user who has downloaded
/// everything, and a Permafrost user who has synced.
async fn setup() -> (FakeDrive, String, String, Fixture, Fixture) {
    let drive = FakeDrive::new("seed@team.org");
    let root = drive.drive_id.clone();
    let gear = drive.seed_file(&root, "Gear.SLDPRT", b"gear v1");
    let robot = drive.seed_file(&root, "Robot.SLDASM", b"robot v1");
    let lite = vault(&drive, "lee@team.org", "lite-pc");
    let perm = vault(&drive, "pat@team.org", "perm-pc");
    perm.vault.sync_down().await.unwrap();
    let p = plan(&lite.vault).await;
    assert_eq!(kinds(&p.download), [("Gear.SLDPRT", Kind::New), ("Robot.SLDASM", Kind::New)]);
    download_all(&lite.vault).await;
    (drive, gear, robot, lite, perm)
}

async fn plan(v: &V) -> LitePlan {
    v.refresh().await.unwrap();
    let p = v.lite_plan().unwrap();
    v.lite_apply_bookkeeping(&p).unwrap();
    p
}

fn kinds(items: &[ChangeItem]) -> Vec<(&str, Kind)> {
    items.iter().map(|i| (i.path.as_str(), i.kind)).collect()
}

fn keys(items: &[ChangeItem]) -> Vec<String> {
    items.iter().map(|i| i.key.clone()).collect()
}

async fn download_all(v: &V) -> super::download::DownloadReport {
    let p = plan(v).await;
    v.lite_download(&keys(&p.download)).await.unwrap()
}

fn write(v: &V, rel: &str, bytes: &[u8]) {
    let p = v.root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn read(v: &V, rel: &str) -> Vec<u8> {
    std::fs::read(v.root.join(rel)).unwrap()
}

#[tokio::test]
async fn downloads_are_writable_and_nothing_is_left_over() {
    let (_drive, _gear, _robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    assert_eq!(read(lv, "Gear.SLDPRT"), b"gear v1");
    assert!(!localfs::is_read_only(&lv.root.join("Gear.SLDPRT")), "Lite never makes files read-only");
    let p = plan(lv).await;
    assert!(p.upload.is_empty() && p.download.is_empty(), "{p:?}");
}

#[tokio::test]
async fn classifies_each_side() {
    let (drive, gear, robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    write(lv, "Gear.SLDPRT", b"gear v2 (lee)");
    drive.external_edit(&robot, b"robot v2 (someone)");
    write(lv, "Bracket.SLDPRT", b"new bracket");
    let p = plan(lv).await;
    assert_eq!(kinds(&p.upload), [("Bracket.SLDPRT", Kind::New), ("Gear.SLDPRT", Kind::Revision)]);
    assert_eq!(kinds(&p.download), [("Robot.SLDASM", Kind::Revision)]);
    assert!(p.upload.iter().all(|i| i.checked));

    // Both sides change the same file: a mismatch on both lists, never ticked.
    drive.external_edit(&gear, b"gear v2 (someone)");
    let p = plan(lv).await;
    let m: Vec<_> = p.upload.iter().chain(&p.download).filter(|i| i.path == "Gear.SLDPRT").collect();
    assert_eq!(m.len(), 2);
    assert!(m.iter().all(|i| i.kind == Kind::Mismatch && !i.checked && i.detail.is_some()));
}

#[tokio::test]
async fn deletes_both_ways() {
    let (drive, gear, robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    std::fs::remove_file(lv.root.join("Robot.SLDASM")).unwrap();
    drive.trash_file(&gear).await.unwrap();
    let p = plan(lv).await;
    assert_eq!(kinds(&p.upload), [("Robot.SLDASM", Kind::Delete)]);
    assert_eq!(kinds(&p.download), [("Gear.SLDPRT", Kind::Delete)]);

    let up = lv.lite_upload(&keys(&p.upload), &[], "").await.unwrap();
    assert_eq!(up.deleted, 1, "{up:?}");
    assert!(drive.get_file(&robot).await.unwrap().trashed, "trashed, so recoverable for 30 days");
    let down = lv.lite_download(&keys(&p.download)).await.unwrap();
    assert_eq!(down.deleted, 1);
    assert!(!lv.root.join("Gear.SLDPRT").exists());
    assert!(lv.trash_dir().read_dir().unwrap().count() == 1, "local copy kept in the app's trash");
    let p = plan(lv).await;
    assert!(p.upload.is_empty() && p.download.is_empty(), "{p:?}");
}

#[tokio::test]
async fn drive_moves_become_one_move_row() {
    let (drive, gear, _robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    let parts = drive.seed_folder(&drive.drive_id, "Parts");
    drive.move_item(&gear, &parts);
    let p = plan(lv).await;
    assert_eq!(kinds(&p.download), [("Parts/Gear.SLDPRT", Kind::Move)]);
    assert!(p.upload.is_empty(), "the old path isn't offered as a new upload: {p:?}");
    let r = lv.lite_download(&keys(&p.download)).await.unwrap();
    assert_eq!(r.moved, 1);
    assert_eq!(read(lv, "Parts/Gear.SLDPRT"), b"gear v1");
    assert!(!lv.root.join("Gear.SLDPRT").exists());
}

#[tokio::test]
async fn first_link_adopts_identical_files_and_flags_different_ones() {
    let drive = FakeDrive::new("seed@team.org");
    let root = drive.drive_id.clone();
    drive.seed_file(&root, "Same.SLDPRT", b"same");
    drive.seed_file(&root, "Differs.SLDPRT", b"drive copy");
    let lite = vault(&drive, "lee@team.org", "lite-pc");
    let lv = &lite.vault;
    write(lv, "Same.SLDPRT", b"same");
    write(lv, "Differs.SLDPRT", b"my copy");
    let p = plan(lv).await;
    assert_eq!(kinds(&p.upload), [("Differs.SLDPRT", Kind::Mismatch)]);
    assert_eq!(kinds(&p.download), [("Differs.SLDPRT", Kind::Mismatch)]);
    // Adopted: a later edit to Same shows up as an ordinary revision.
    write(lv, "Same.SLDPRT", b"same, edited");
    let p = plan(lv).await;
    assert!(p.upload.iter().any(|i| i.path == "Same.SLDPRT" && i.kind == Kind::Revision), "{p:?}");
}

#[tokio::test]
async fn uploads_look_like_a_normal_push_to_permafrost() {
    let (drive, gear, _robot, lite, perm) = setup().await;
    let lv = &lite.vault;
    write(lv, "Gear.SLDPRT", b"gear v2");
    write(lv, "Sub/Spacer.SLDPRT", b"spacer");
    let p = plan(lv).await;
    let up = lv.lite_upload(&keys(&p.upload), &[], "new spacer").await.unwrap();
    assert_eq!((up.uploaded, up.skipped.len()), (2, 0), "{up:?}");
    let cs = up.changeset.unwrap();

    let f = drive.get_file(&gear).await.unwrap();
    assert!(f.lock().is_none() && !f.read_only && f.prop(props::PENDING_CHANGESET).is_none());
    assert!(drive.is_kept_forever(&gear, f.head_revision_id.as_deref().unwrap()));
    let spacer = drive.files_named("Spacer.SLDPRT");
    assert!(spacer[0].lock().is_none(), "new files aren't checked out to anyone");

    let pv = &perm.vault;
    let r = pv.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 2, "{r:?}");
    assert_eq!(std::fs::read(pv.root.join("Sub/Spacer.SLDPRT")).unwrap(), b"spacer");
    let m = pv.index.changeset(&cs).unwrap().expect("in Permafrost's History");
    assert_eq!(m.message, "new spacer");
    let p = plan(lv).await;
    assert!(p.upload.is_empty() && p.download.is_empty(), "{p:?}");
}

#[tokio::test]
async fn mismatch_needs_overwrite_and_keeps_the_other_version() {
    let (drive, gear, _robot, lite, perm) = setup().await;
    let (lv, pv) = (&lite.vault, &perm.vault);
    pv.check_out(&gear).await.unwrap();
    std::fs::write(pv.root.join("Gear.SLDPRT"), b"gear (pat)").unwrap();
    pv.push(PushRequest { files: vec![gear.clone()], message: "pat".into(), ..Default::default() }).await.unwrap();
    write(lv, "Gear.SLDPRT", b"gear (lee)");

    let p = plan(lv).await;
    let row = p.upload.iter().find(|i| i.path == "Gear.SLDPRT").unwrap();
    assert_eq!(row.kind, Kind::Mismatch);
    let up = lv.lite_upload(&[row.key.clone()], &[], "").await.unwrap();
    assert_eq!(up.uploaded, 0);
    assert!(up.skipped[0].reason.contains("download it first"), "{up:?}");
    assert_eq!(drive.content(&gear).unwrap(), b"gear (pat)");

    let revs = drive.revision_count(&gear);
    let up = lv.lite_upload(&[row.key.clone()], &[row.key.clone()], "").await.unwrap();
    assert_eq!(up.uploaded, 1, "{up:?}");
    assert_eq!(drive.content(&gear).unwrap(), b"gear (lee)");
    assert_eq!(drive.revision_count(&gear), revs + 1, "Pat's version is still in Drive's history");
}

#[tokio::test]
async fn never_overwrites_a_file_checked_out_in_permafrost() {
    let (drive, gear, _robot, lite, perm) = setup().await;
    let (lv, pv) = (&lite.vault, &perm.vault);
    pv.check_out(&gear).await.unwrap();
    write(lv, "Gear.SLDPRT", b"gear (lee)");
    let p = plan(lv).await;
    let row = p.upload.iter().find(|i| i.path == "Gear.SLDPRT").unwrap();
    assert!(row.blocked.as_deref().unwrap().contains("Checked out in Permafrost"), "{row:?}");
    let up = lv.lite_upload(&[row.key.clone()], &[row.key.clone()], "").await.unwrap();
    assert_eq!(up.uploaded, 0);
    assert_eq!(drive.content(&gear).unwrap(), b"gear v1");

    // Pat's push still arrives as a download (Lee's edit makes it a mismatch).
    std::fs::write(pv.root.join("Gear.SLDPRT"), b"gear (pat)").unwrap();
    pv.push(PushRequest { files: vec![gear.clone()], message: "pat".into(), keep_locked: true, ..Default::default() })
        .await
        .unwrap();
    let p = plan(lv).await;
    let down = p.download.iter().find(|i| i.path == "Gear.SLDPRT").unwrap();
    assert_eq!(down.kind, Kind::Mismatch);
    let r = lv.lite_download(&[down.key.clone()]).await.unwrap();
    assert_eq!(r.downloaded, 1);
    assert_eq!(read(lv, "Gear.SLDPRT"), b"gear (pat)");
    assert_eq!(std::fs::read(&r.backups[0]).unwrap(), b"gear (lee)", "Lee's edit was backed up");
    assert!(!r.backups[0].starts_with(&lv.root), "backups live outside the project");
}

#[tokio::test]
async fn a_change_between_listing_and_uploading_is_left_out() {
    let (drive, gear, robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    write(lv, "Gear.SLDPRT", b"gear (lee)");
    write(lv, "Robot.SLDASM", b"robot (lee)");
    let p = plan(lv).await;
    drive.external_edit(&gear, b"gear (someone, just now)");
    // The row was a plain revision when listed; the push re-checks before uploading.
    let up = lv.lite_upload(&keys(&p.upload), &[], "").await.unwrap();
    assert_eq!(up.uploaded, 1, "{up:?}");
    assert_eq!(drive.content(&gear).unwrap(), b"gear (someone, just now)");
    assert_eq!(drive.content(&robot).unwrap(), b"robot (lee)");
    assert!(up.skipped.iter().any(|s| s.rel_path.ends_with("Gear.SLDPRT")), "{up:?}");
}

#[tokio::test]
async fn interrupted_upload_resumes_without_duplicates() {
    let (drive, _gear, _robot, lite, perm) = setup().await;
    let lv = &lite.vault;
    write(lv, "Spacer.SLDPRT", b"spacer");
    let p = plan(lv).await;
    drive.lose_next_create_responses(1); // created on Drive, but the app never heard back
    assert!(lv.lite_upload(&keys(&p.upload), &[], "").await.is_err());
    assert_eq!(lv.index.journal().unwrap().len(), 1, "journal kept for resuming");

    let done = lv.resume_pushes().await.unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(drive.files_named("Spacer.SLDPRT").len(), 1, "found the file, didn't create another");
    let r = perm.vault.sync_down().await.unwrap();
    assert_eq!(r.downloaded, 1, "{r:?}");
    assert!(!perm.vault.root.join("Spacer (2).SLDPRT").exists());
}

#[tokio::test]
async fn failed_downloads_retry_and_never_half_apply() {
    let (drive, gear, robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    drive.external_edit(&gear, b"gear v2");
    drive.external_edit(&robot, b"robot v2");
    drive.fail_next_downloads(2);
    let r = download_all(lv).await;
    assert_eq!(r.downloaded, 2, "retried past two failures");

    drive.external_edit(&gear, b"gear v3");
    drive.external_edit(&robot, b"robot v3");
    drive.fail_next_downloads(10);
    let p = plan(lv).await;
    assert!(lv.lite_download(&keys(&p.download)).await.is_err());
    assert_eq!(read(lv, "Gear.SLDPRT"), b"gear v2", "nothing swapped in");
    assert_eq!(read(lv, "Robot.SLDASM"), b"robot v2");
}

#[tokio::test]
async fn abandoned_pending_tags_stop_holding_files_back() {
    let (drive, gear, _robot, lite, _perm) = setup().await;
    let lv = &lite.vault;
    let old_ms = (chrono::Utc::now().timestamp_millis() - 3 * 24 * 3_600_000) as u64;
    let old = ulid::Ulid::from_parts(old_ms, 1).to_string();
    let fresh = ulid::Ulid::new().to_string();
    drive.external_edit(&gear, b"gear v2");
    drive.update_metadata(&gear, MetadataPatch::default().set(props::PENDING_CHANGESET, &fresh)).await.unwrap();
    assert!(plan(lv).await.download.is_empty(), "a push in progress holds the file back");
    drive.update_metadata(&gear, MetadataPatch::default().set(props::PENDING_CHANGESET, &old)).await.unwrap();
    assert_eq!(kinds(&plan(lv).await.download), [("Gear.SLDPRT", Kind::Revision)]);
}
