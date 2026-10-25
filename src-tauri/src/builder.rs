//! Writes one system software file to `PS5/UPDATE/PS5UPDATE.PUP` on a drive
//! that has already been identified and checked.
//!
//! The builder never formats anything and only creates or removes files whose
//! names carry this run's id. An existing `PS5UPDATE.PUP` is kept as a `.bak`
//! until the new file has been written, flushed, re-read and re-hashed,
//! because renames on FAT32 and exFAT are not crash-atomic.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::integrity::{self, FAT32_MAX_FILE, HashError, LocalHashes};
use crate::volumes::VolumeInfo;

pub const TARGET_NAME: &str = "PS5UPDATE.PUP";
/// Headroom on top of the file size for the file system's own bookkeeping.
pub const SPACE_MARGIN: u64 = 64 * 1024 * 1024;
const BUF_SIZE: usize = 1024 * 1024;
const PROGRESS_STEP: u64 = 8 * 1024 * 1024;

pub fn target_dir(root: &Path) -> PathBuf {
    root.join("PS5").join("UPDATE")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preflight {
    pub target_path: String,
    pub target_exists: bool,
    pub target_size: Option<u64>,
    /// Other `.PUP` files in the folder; the console expects exactly one.
    pub other_pups: Vec<String>,
    pub bytes_needed: u64,
    pub blockers: Vec<String>,
}

/// Facts and blockers for writing a file of `source_size` bytes to `volume`.
/// `root` is the volume's mount path (a temp folder in tests).
pub fn preflight(source_size: u64, volume: &VolumeInfo, root: &Path) -> Preflight {
    let mut blockers = volume.blockers();
    if source_size == 0 {
        blockers.push("The file is empty.".into());
    }
    if volume.is_fat32() && source_size > FAT32_MAX_FILE {
        blockers.push("The file is larger than 4 GiB, which FAT32 cannot store.".into());
    }
    let bytes_needed = source_size.saturating_add(SPACE_MARGIN);
    if volume.free_bytes < bytes_needed {
        blockers.push(format!(
            "Not enough free space: {} bytes needed (file plus a safety margin), {} free.",
            bytes_needed, volume.free_bytes
        ));
    }
    let dir = target_dir(root);
    let target = dir.join(TARGET_NAME);
    let existing = fs::metadata(&target).ok().filter(|m| m.is_file());
    if dir.exists() && !dir.is_dir() {
        blockers.push("PS5\\UPDATE exists on the drive but is not a folder.".into());
    }
    Preflight {
        target_path: format!("PS5\\UPDATE\\{TARGET_NAME}"),
        target_exists: existing.is_some(),
        target_size: existing.map(|m| m.len()),
        other_pups: other_pups(&dir),
        bytes_needed,
        blockers,
    }
}

fn other_pups(dir: &Path) -> Vec<String> {
    let Ok(read) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<String> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".pup") && !n.eq_ignore_ascii_case(TARGET_NAME))
        .collect();
    out.sort();
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Reading the source and writing the temporary file.
    Writing,
    /// Re-reading the temporary file from the drive.
    CheckingTemp,
    /// Re-reading the final file.
    CheckingFinal,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Cancelled. Nothing was changed on the drive.")]
    Cancelled,
    #[error("{0}")]
    Rejected(String),
    #[error(
        "PS5UPDATE.PUP already exists on the drive. Choose \"Replace\" to keep the old file as a backup while the new one is written."
    )]
    TargetExists,
    #[error("The copy on the drive did not match the source, so it was removed. Try again or use another drive.")]
    VerifyFailed,
    #[error("{0}")]
    Io(String),
}

impl From<io::Error> for BuildError {
    fn from(e: io::Error) -> Self {
        BuildError::Io(format!("Disk error: {e}"))
    }
}

impl From<HashError> for BuildError {
    fn from(e: HashError) -> Self {
        match e {
            HashError::Cancelled => BuildError::Cancelled,
            HashError::Io(e) => e.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildOutcome {
    pub local: LocalHashes,
    pub replaced_existing: bool,
}

/// A run id is 8 lowercase hex digits; file names carry it so recovery can
/// tell this app's leftovers from anyone else's files.
pub fn new_run_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:08x}", (nanos as u64 ^ (u64::from(std::process::id()) << 20)) as u32)
}

fn temp_name(run_id: &str) -> String {
    format!("{TARGET_NAME}.{run_id}.tmp")
}

fn backup_name(run_id: &str) -> String {
    format!("{TARGET_NAME}.{run_id}.bak")
}

/// What the build has touched, so a failure can undo exactly that.
#[derive(Default)]
struct Touched {
    tmp: bool,
    bak: bool,
    final_in_place: bool,
}

/// Write `source` to `root/PS5/UPDATE/PS5UPDATE.PUP`.
///
/// `validate` sees the hashes computed while copying and must accept them
/// before anything is moved into place (the caller compares them with the
/// catalog). `progress(phase, done, total)` is called every few MiB.
pub fn build(
    source: &Path,
    root: &Path,
    run_id: &str,
    replace_existing: bool,
    cancel: &AtomicBool,
    validate: impl FnOnce(&LocalHashes) -> Result<(), String>,
    mut progress: impl FnMut(Phase, u64, u64),
) -> Result<BuildOutcome, BuildError> {
    assert!(run_id.len() == 8 && run_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let dir = target_dir(root);
    let tmp = dir.join(temp_name(run_id));
    let bak = dir.join(backup_name(run_id));
    let target = dir.join(TARGET_NAME);
    let mut touched = Touched::default();

    let result =
        run(source, &dir, &tmp, &bak, &target, replace_existing, cancel, validate, &mut progress, &mut touched);
    if result.is_err() {
        undo(&tmp, &bak, &target, &touched);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run(
    source: &Path,
    dir: &Path,
    tmp: &Path,
    bak: &Path,
    target: &Path,
    replace_existing: bool,
    cancel: &AtomicBool,
    validate: impl FnOnce(&LocalHashes) -> Result<(), String>,
    progress: &mut impl FnMut(Phase, u64, u64),
    touched: &mut Touched,
) -> Result<BuildOutcome, BuildError> {
    let existing = target.is_file();
    if existing && !replace_existing {
        return Err(BuildError::TargetExists);
    }
    fs::create_dir_all(dir)?;

    // Write the temporary file, hashing what is read from the source.
    let mut src = File::open(source)?;
    let total = src.metadata()?.len();
    let mut out = OpenOptions::new().write(true).create_new(true).open(tmp)?;
    touched.tmp = true;
    let local = copy_hashing(&mut src, &mut out, total, cancel, &mut |d, t| progress(Phase::Writing, d, t))?;
    out.flush()?;
    out.sync_all()?;
    drop(out);

    validate(&local).map_err(BuildError::Rejected)?;

    // Re-open the temporary file and hash what is actually on the drive.
    let on_disk = integrity::hash_file(tmp, cancel, |d, t| progress(Phase::CheckingTemp, d, t))?;
    if on_disk != local {
        return Err(BuildError::VerifyFailed);
    }

    if existing {
        fs::rename(target, bak)?;
        touched.bak = true;
    }
    fs::rename(tmp, target)?;
    touched.tmp = false;
    touched.final_in_place = true;

    let final_hash = integrity::hash_file(target, cancel, |d, t| progress(Phase::CheckingFinal, d, t))?;
    if final_hash != local {
        return Err(BuildError::VerifyFailed);
    }
    if touched.bak {
        // The new file is verified; the backup has done its job.
        let _ = fs::remove_file(bak);
        touched.bak = false;
    }
    Ok(BuildOutcome { local, replaced_existing: existing })
}

/// Remove only what this run created and put a moved original back.
fn undo(tmp: &Path, bak: &Path, target: &Path, touched: &Touched) {
    if touched.tmp {
        let _ = fs::remove_file(tmp);
    }
    if touched.final_in_place {
        let _ = fs::remove_file(target);
    }
    if touched.bak {
        let _ = fs::rename(bak, target);
    }
}

fn copy_hashing(
    src: &mut impl Read,
    dst: &mut impl Write,
    total: u64,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<LocalHashes, BuildError> {
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut buf = vec![0u8; BUF_SIZE];
    let mut done = 0u64;
    let mut next_report = PROGRESS_STEP;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(BuildError::Cancelled);
        }
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        sha.update(&buf[..n]);
        md5.update(&buf[..n]);
        dst.write_all(&buf[..n])?;
        done += n as u64;
        if done >= next_report {
            progress(done, total);
            next_report = done + PROGRESS_STEP;
        }
    }
    progress(done, total);
    Ok(LocalHashes { size_bytes: done, sha256: hex::encode(sha.finalize()), md5: hex::encode(md5.finalize()) })
}

// ---------------------------------------------------------------------------
// Leftovers from an interrupted run
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftoverKind {
    /// A partly written new file.
    Temp,
    /// The previous PS5UPDATE.PUP, set aside during a replace.
    Backup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftoverAction {
    Delete,
    /// Put a backup back as PS5UPDATE.PUP (only when that name is free).
    Restore,
}

#[derive(Debug, Clone, Serialize)]
pub struct Leftover {
    pub file_name: String,
    pub kind: LeftoverKind,
    pub size: u64,
    /// Whether a PS5UPDATE.PUP exists now.
    pub final_exists: bool,
}

/// Recognise `PS5UPDATE.PUP.<8 hex>.tmp|bak`; nothing else counts as ours.
pub fn parse_leftover(name: &str) -> Option<LeftoverKind> {
    let lower = name.to_ascii_lowercase();
    let rest = lower.strip_prefix("ps5update.pup.")?;
    let (id, ext) = rest.split_once('.')?;
    if id.len() != 8 || !id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    match ext {
        "tmp" => Some(LeftoverKind::Temp),
        "bak" => Some(LeftoverKind::Backup),
        _ => None,
    }
}

pub fn find_leftovers(root: &Path) -> Vec<Leftover> {
    let dir = target_dir(root);
    let final_exists = dir.join(TARGET_NAME).is_file();
    let Ok(read) = fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Leftover> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let kind = parse_leftover(&name)?;
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            Some(Leftover { file_name: name, kind, size: meta.len(), final_exists })
        })
        .collect();
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    out
}

pub fn resolve_leftover(root: &Path, file_name: &str, action: LeftoverAction) -> Result<String, String> {
    let kind = parse_leftover(file_name).ok_or("That is not a file this app created.")?;
    let dir = target_dir(root);
    let path = dir.join(file_name);
    if !path.is_file() {
        return Err("That file is no longer there.".into());
    }
    match action {
        LeftoverAction::Delete => {
            fs::remove_file(&path).map_err(|e| format!("Could not delete it: {e}"))?;
            Ok("Deleted.".into())
        }
        LeftoverAction::Restore => {
            if kind != LeftoverKind::Backup {
                return Err("Only a backup can be restored. A partly written file can only be deleted.".into());
            }
            let target = dir.join(TARGET_NAME);
            if target.exists() {
                return Err(
                    "PS5UPDATE.PUP already exists, so the backup was not restored. Delete the backup instead.".into()
                );
            }
            fs::rename(&path, &target).map_err(|e| format!("Could not restore it: {e}"))?;
            Ok("Restored as PS5UPDATE.PUP.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::fake::usb_stick;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn setup(content: &[u8]) -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
        let src_dir = tempfile::tempdir().unwrap();
        let drive = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("source.PUP");
        fs::write(&src, content).unwrap();
        (src_dir, drive, src)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    const ID: &str = "0badf00d";

    #[test]
    fn writes_the_file_and_leaves_nothing_else() {
        let data = vec![7u8; 3 * 1024 * 1024 + 5];
        let (_s, drive, src) = setup(&data);
        let out = build(&src, drive.path(), ID, false, &no_cancel(), |_| Ok(()), |_, _, _| {}).unwrap();
        assert_eq!(out.local.size_bytes, data.len() as u64);
        assert!(!out.replaced_existing);
        let dir = target_dir(drive.path());
        assert_eq!(names(&dir), vec![TARGET_NAME.to_string()]);
        assert_eq!(fs::read(dir.join(TARGET_NAME)).unwrap(), data);
    }

    #[test]
    fn refuses_to_overwrite_without_replace_and_changes_nothing() {
        let (_s, drive, src) = setup(b"new");
        let dir = target_dir(drive.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(TARGET_NAME), b"old").unwrap();
        let err = build(&src, drive.path(), ID, false, &no_cancel(), |_| Ok(()), |_, _, _| {}).unwrap_err();
        assert!(matches!(err, BuildError::TargetExists));
        assert_eq!(names(&dir), vec![TARGET_NAME.to_string()]);
        assert_eq!(fs::read(dir.join(TARGET_NAME)).unwrap(), b"old");
    }

    #[test]
    fn replace_swaps_the_file_and_deletes_the_backup_only_after_verification() {
        let (_s, drive, src) = setup(b"new contents");
        let dir = target_dir(drive.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(TARGET_NAME), b"old").unwrap();
        let out = build(&src, drive.path(), ID, true, &no_cancel(), |_| Ok(()), |_, _, _| {}).unwrap();
        assert!(out.replaced_existing);
        assert_eq!(names(&dir), vec![TARGET_NAME.to_string()]);
        assert_eq!(fs::read(dir.join(TARGET_NAME)).unwrap(), b"new contents");
    }

    #[test]
    fn rejected_hashes_remove_the_temp_file_and_keep_the_original() {
        let (_s, drive, src) = setup(b"not the right file");
        let dir = target_dir(drive.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(TARGET_NAME), b"old").unwrap();
        let err =
            build(&src, drive.path(), ID, true, &no_cancel(), |_| Err("wrong file".into()), |_, _, _| {}).unwrap_err();
        assert!(matches!(err, BuildError::Rejected(m) if m == "wrong file"));
        assert_eq!(names(&dir), vec![TARGET_NAME.to_string()]);
        assert_eq!(fs::read(dir.join(TARGET_NAME)).unwrap(), b"old");
    }

    #[test]
    fn cancel_removes_the_temp_file() {
        // Cancel from the progress callback, part-way through the copy.
        let (_s, drive, src) = setup(&vec![1u8; 20 * 1024 * 1024]);
        let cancel = AtomicBool::new(false);
        let err =
            build(&src, drive.path(), ID, false, &cancel, |_| Ok(()), |_, _, _| cancel.store(true, Ordering::SeqCst))
                .unwrap_err();
        assert!(matches!(err, BuildError::Cancelled));
        assert!(names(&target_dir(drive.path())).is_empty());
    }

    #[test]
    fn missing_source_leaves_the_drive_untouched() {
        let drive = tempfile::tempdir().unwrap();
        let err =
            build(Path::new("does-not-exist.PUP"), drive.path(), ID, false, &no_cancel(), |_| Ok(()), |_, _, _| {})
                .unwrap_err();
        assert!(matches!(err, BuildError::Io(_)));
        assert!(names(&target_dir(drive.path())).is_empty());
    }

    #[test]
    fn a_leftover_temp_name_from_the_same_run_is_never_overwritten() {
        let (_s, drive, src) = setup(b"data");
        let dir = target_dir(drive.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(temp_name(ID)), b"someone else's").unwrap();
        let err = build(&src, drive.path(), ID, false, &no_cancel(), |_| Ok(()), |_, _, _| {}).unwrap_err();
        assert!(matches!(err, BuildError::Io(_)));
    }

    #[test]
    fn undo_restores_the_original_after_a_late_failure() {
        let dir = tempfile::tempdir().unwrap();
        let (tmp, bak, target) = (dir.path().join("t.tmp"), dir.path().join("t.bak"), dir.path().join("T.PUP"));
        fs::write(&target, b"half-verified new file").unwrap();
        fs::write(&bak, b"original").unwrap();
        undo(&tmp, &bak, &target, &Touched { tmp: false, bak: true, final_in_place: true });
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert!(!bak.exists());
    }

    #[test]
    fn preflight_blocks_on_size_space_and_filesystem() {
        let root = tempfile::tempdir().unwrap();
        let mut v = usb_stick("a", "E:\\");
        assert!(preflight(1 << 30, &v, root.path()).blockers.is_empty());

        v.free_bytes = (1 << 30) + 1024;
        assert!(preflight(1 << 30, &v, root.path()).blockers[0].contains("Not enough free space"));

        v.free_bytes = 100 << 30;
        v.filesystem = "FAT32".into();
        assert!(preflight(5 << 30, &v, root.path()).blockers[0].contains("4 GiB"));
        v.filesystem = "EXFAT".into();
        assert!(preflight(5 << 30, &v, root.path()).blockers.is_empty());

        v.filesystem = "NTFS".into();
        assert!(!preflight(1 << 30, &v, root.path()).blockers.is_empty());
        assert!(!preflight(0, &usb_stick("a", "E:\\"), root.path()).blockers.is_empty());
    }

    #[test]
    fn preflight_reports_existing_target_and_other_pups() {
        let root = tempfile::tempdir().unwrap();
        let dir = target_dir(root.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(TARGET_NAME), b"old").unwrap();
        fs::write(dir.join("other.pup"), b"x").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        let p = preflight(1 << 20, &usb_stick("a", "E:\\"), root.path());
        assert!(p.target_exists);
        assert_eq!(p.target_size, Some(3));
        assert_eq!(p.other_pups, vec!["other.pup".to_string()]);
    }

    #[test]
    fn leftover_names_must_match_the_pattern_exactly() {
        assert_eq!(parse_leftover("PS5UPDATE.PUP.0badf00d.tmp"), Some(LeftoverKind::Temp));
        assert_eq!(parse_leftover("ps5update.pup.0BADF00D.BAK"), Some(LeftoverKind::Backup));
        assert_eq!(parse_leftover("PS5UPDATE.PUP"), None);
        assert_eq!(parse_leftover("PS5UPDATE.PUP.0badf00.tmp"), None);
        assert_eq!(parse_leftover("PS5UPDATE.PUP.0badf00g.tmp"), None);
        assert_eq!(parse_leftover("PS5UPDATE.PUP.0badf00d.tmp.exe"), None);
        assert_eq!(parse_leftover("..\\PS5UPDATE.PUP.0badf00d.tmp"), None);
        assert_eq!(parse_leftover("other.0badf00d.tmp"), None);
    }

    #[test]
    fn resolving_leftovers_only_touches_matching_files() {
        let root = tempfile::tempdir().unwrap();
        let dir = target_dir(root.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("PS5UPDATE.PUP.11111111.bak"), b"orig").unwrap();
        fs::write(dir.join("PS5UPDATE.PUP.22222222.tmp"), b"part").unwrap();
        fs::write(dir.join("keep.txt"), b"mine").unwrap();

        let found = find_leftovers(root.path());
        assert_eq!(found.len(), 2);
        assert!(!found[0].final_exists);

        assert!(resolve_leftover(root.path(), "keep.txt", LeftoverAction::Delete).is_err());
        assert!(resolve_leftover(root.path(), "..\\x", LeftoverAction::Delete).is_err());
        assert!(resolve_leftover(root.path(), "PS5UPDATE.PUP.22222222.tmp", LeftoverAction::Restore).is_err());

        resolve_leftover(root.path(), "PS5UPDATE.PUP.11111111.bak", LeftoverAction::Restore).unwrap();
        assert_eq!(fs::read(dir.join(TARGET_NAME)).unwrap(), b"orig");

        // A second backup can't clobber the restored file.
        fs::write(dir.join("PS5UPDATE.PUP.33333333.bak"), b"older").unwrap();
        assert!(resolve_leftover(root.path(), "PS5UPDATE.PUP.33333333.bak", LeftoverAction::Restore).is_err());
        resolve_leftover(root.path(), "PS5UPDATE.PUP.33333333.bak", LeftoverAction::Delete).unwrap();
        resolve_leftover(root.path(), "PS5UPDATE.PUP.22222222.tmp", LeftoverAction::Delete).unwrap();
        assert_eq!(names(&dir), vec![TARGET_NAME.to_string(), "keep.txt".to_string()]);
    }

    #[test]
    fn run_ids_have_the_expected_shape() {
        let id = new_run_id();
        assert_eq!(id.len(), 8);
        assert!(id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }
}
