//! Storage / path notebook: read-only snapshots of folders the user picks,
//! comparison between two snapshots, and redacted JSON/CSV export.
//!
//! The app only reads folder listings (names, sizes, modified times). It never
//! scans anything the user did not choose, never follows links out of the
//! chosen folder, and cannot see the PS5's internal database. A snapshot of a
//! USB drive shows what a PC sees on it, nothing more.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_ENTRIES: usize = 200_000;
pub const MAX_DEPTH: usize = 32;
pub const MAX_SNAPSHOTS: usize = 30;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Largest list of changes sent to the UI; counts are always exact.
pub const MAX_DIFF_ROWS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Path relative to the chosen folder, `/`-separated.
    pub path: String,
    pub dir: bool,
    pub size: u64,
    /// Modified time, Unix seconds.
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A folder on this computer.
    PcFolder,
    /// A USB drive meant for the console, as the computer sees it.
    ConsoleUsb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub taken_at: String,
    pub label: String,
    pub note: String,
    pub kind: Kind,
    pub root: String,
    pub entries: Vec<Entry>,
    pub file_count: u64,
    pub dir_count: u64,
    pub total_bytes: u64,
    /// The scan stopped at the entry or depth limit.
    pub truncated: bool,
    /// Links and unreadable folders that were skipped.
    pub skipped: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub taken_at: String,
    pub label: String,
    pub note: String,
    pub kind: Kind,
    pub root: String,
    pub file_count: u64,
    pub dir_count: u64,
    pub total_bytes: u64,
    pub truncated: bool,
    pub skipped: u64,
}

impl Snapshot {
    pub fn summary(&self) -> SnapshotSummary {
        SnapshotSummary {
            id: self.id.clone(),
            taken_at: self.taken_at.clone(),
            label: self.label.clone(),
            note: self.note.clone(),
            kind: self.kind,
            root: self.root.clone(),
            file_count: self.file_count,
            dir_count: self.dir_count,
            total_bytes: self.total_bytes,
            truncated: self.truncated,
            skipped: self.skipped,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Notebook {
    pub schema_version: u32,
    pub snapshots: Vec<Snapshot>,
}

impl Notebook {
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return (Self { schema_version: SCHEMA_VERSION, snapshots: Vec::new() }, None),
        };
        let empty = || Self { schema_version: SCHEMA_VERSION, snapshots: Vec::new() };
        if meta.len() > MAX_FILE_BYTES {
            return (empty(), Some("The notebook file was too large and was ignored.".into()));
        }
        match fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Self>(&t).map_err(|e| e.to_string()))
        {
            Ok(n) if n.schema_version == SCHEMA_VERSION => (n, None),
            Ok(_) => (empty(), Some("The notebook is from another app version and was ignored.".into())),
            Err(e) => (empty(), Some(format!("The notebook could not be read and was ignored ({e})."))),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_atomic(path, &serde_json::to_vec(self).map_err(std::io::Error::other)?)
    }

    pub fn add(&mut self, snap: Snapshot) -> Result<(), String> {
        if self.snapshots.len() >= MAX_SNAPSHOTS {
            return Err(format!("The notebook holds at most {MAX_SNAPSHOTS} snapshots. Delete one first."));
        }
        self.snapshots.push(snap);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }
}

/// Write via a temp file in the same folder, then rename over the target.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("Scan cancelled.")]
    Cancelled,
    #[error("{0}")]
    Io(String),
}

pub struct ScanResult {
    pub entries: Vec<Entry>,
    pub truncated: bool,
    pub skipped: u64,
}

#[cfg(windows)]
fn is_link(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const REPARSE_POINT: u32 = 0x400;
    meta.file_type().is_symlink() || meta.file_attributes() & REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

fn modified_secs(meta: &fs::Metadata) -> Option<i64> {
    let m = meta.modified().ok()?;
    Some(match m.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    })
}

/// List everything under `root`, without following links.
pub fn scan(root: &Path, cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> Result<ScanResult, ScanError> {
    let meta = fs::symlink_metadata(root).map_err(|e| ScanError::Io(format!("Can't read that folder: {e}")))?;
    if !meta.is_dir() || is_link(&meta) {
        return Err(ScanError::Io("Choose a folder (not a link or a file).".into()));
    }
    let mut entries = Vec::new();
    let mut skipped = 0u64;
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
    'outer: while let Some((dir, prefix, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        for item in read.flatten() {
            let Ok(meta) = fs::symlink_metadata(item.path()) else {
                skipped += 1;
                continue;
            };
            if is_link(&meta) {
                skipped += 1;
                continue;
            }
            if entries.len() >= MAX_ENTRIES {
                truncated = true;
                break 'outer;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if meta.is_dir() {
                entries.push(Entry { path: rel.clone(), dir: true, size: 0, modified: modified_secs(&meta) });
                if depth + 1 >= MAX_DEPTH {
                    truncated = true;
                } else {
                    stack.push((item.path(), rel, depth + 1));
                }
            } else if meta.is_file() {
                entries.push(Entry { path: rel, dir: false, size: meta.len(), modified: modified_secs(&meta) });
            }
            if entries.len() % 1000 == 0 {
                progress(entries.len() as u64);
            }
        }
    }
    progress(entries.len() as u64);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ScanResult { entries, truncated, skipped })
}

pub fn snapshot_from(
    id: String,
    taken_at: String,
    label: String,
    note: String,
    kind: Kind,
    root: &Path,
    scanned: ScanResult,
) -> Snapshot {
    let file_count = scanned.entries.iter().filter(|e| !e.dir).count() as u64;
    let dir_count = scanned.entries.len() as u64 - file_count;
    let total_bytes = scanned.entries.iter().map(|e| e.size).sum();
    Snapshot {
        id,
        taken_at,
        label,
        note,
        kind,
        root: root.to_string_lossy().into_owned(),
        entries: scanned.entries,
        file_count,
        dir_count,
        total_bytes,
        truncated: scanned.truncated,
        skipped: scanned.skipped,
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub path: String,
    pub old_size: u64,
    pub new_size: u64,
    pub old_modified: Option<i64>,
    pub new_modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Move {
    pub from: String,
    pub to: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Diff {
    pub added: Vec<Entry>,
    pub removed: Vec<Entry>,
    pub changed: Vec<Change>,
    /// Files that disappeared from one path and appeared at another with the
    /// same size and modified time. A guess, labelled as one in the UI.
    pub moved: Vec<Move>,
    pub unchanged: u64,
    pub added_total: u64,
    pub removed_total: u64,
    pub changed_total: u64,
    pub moved_total: u64,
    /// Lists were cut at `MAX_DIFF_ROWS`; totals are exact.
    pub cut: bool,
}

/// Windows paths are case-insensitive, so `Photos` and `photos` are one path.
fn key(path: &str) -> String {
    path.to_lowercase()
}

pub fn compare(old: &Snapshot, new: &Snapshot) -> Diff {
    let old_map: HashMap<String, &Entry> = old.entries.iter().map(|e| (key(&e.path), e)).collect();
    let new_map: HashMap<String, &Entry> = new.entries.iter().map(|e| (key(&e.path), e)).collect();

    let mut added: Vec<&Entry> = new.entries.iter().filter(|e| !old_map.contains_key(&key(&e.path))).collect();
    let mut removed: Vec<&Entry> = old.entries.iter().filter(|e| !new_map.contains_key(&key(&e.path))).collect();
    let mut changed = Vec::new();
    let mut unchanged = 0u64;
    for e in &new.entries {
        if let Some(o) = old_map.get(&key(&e.path)) {
            if o.dir != e.dir || (!e.dir && (o.size != e.size || o.modified != e.modified)) {
                changed.push(Change {
                    path: e.path.clone(),
                    old_size: o.size,
                    new_size: e.size,
                    old_modified: o.modified,
                    new_modified: e.modified,
                });
            } else {
                unchanged += 1;
            }
        }
    }

    // Pair removed and added files that look identical (size + modified time),
    // only when each such signature is unique on both sides.
    let sig = |e: &&Entry| (e.size, e.modified);
    let mut rem_sigs: HashMap<(u64, Option<i64>), Vec<usize>> = HashMap::new();
    let mut add_sigs: HashMap<(u64, Option<i64>), Vec<usize>> = HashMap::new();
    for (i, e) in removed.iter().enumerate().filter(|(_, e)| !e.dir && e.size > 0) {
        rem_sigs.entry(sig(e)).or_default().push(i);
    }
    for (i, e) in added.iter().enumerate().filter(|(_, e)| !e.dir && e.size > 0) {
        add_sigs.entry(sig(e)).or_default().push(i);
    }
    let mut moved = Vec::new();
    let (mut drop_rem, mut drop_add) = (Vec::new(), Vec::new());
    for (s, rs) in &rem_sigs {
        if rs.len() != 1 {
            continue;
        }
        let Some(a) = add_sigs.get(s).filter(|a| a.len() == 1) else { continue };
        let (r, ad) = (rs[0], a[0]);
        moved.push(Move { from: removed[r].path.clone(), to: added[ad].path.clone(), size: s.0 });
        drop_rem.push(r);
        drop_add.push(ad);
    }
    let mut i = 0;
    removed.retain(|_| {
        i += 1;
        !drop_rem.contains(&(i - 1))
    });
    let mut j = 0;
    added.retain(|_| {
        j += 1;
        !drop_add.contains(&(j - 1))
    });
    moved.sort_by(|a, b| a.from.cmp(&b.from));
    changed.sort_by(|a, b| a.path.cmp(&b.path));

    let mut diff = Diff {
        added_total: added.len() as u64,
        removed_total: removed.len() as u64,
        changed_total: changed.len() as u64,
        moved_total: moved.len() as u64,
        unchanged,
        ..Default::default()
    };
    diff.cut = added.len() > MAX_DIFF_ROWS
        || removed.len() > MAX_DIFF_ROWS
        || changed.len() > MAX_DIFF_ROWS
        || moved.len() > MAX_DIFF_ROWS;
    diff.added = added.into_iter().take(MAX_DIFF_ROWS).cloned().collect();
    diff.removed = removed.into_iter().take(MAX_DIFF_ROWS).cloned().collect();
    diff.changed = changed.into_iter().take(MAX_DIFF_ROWS).collect();
    diff.moved = moved.into_iter().take(MAX_DIFF_ROWS).collect();
    diff
}

// ---------------------------------------------------------------------------
// Export with redaction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Json,
    Csv,
}

/// Replace the account name wherever it appears in `text`, ignoring case.
pub fn redact_user(text: &str, username: &str) -> String {
    if username.chars().count() < 2 {
        return text.to_string();
    }
    let lower = text.to_lowercase();
    let needle = username.to_lowercase();
    // Lowercasing can change byte lengths for a few exotic characters, which would
    // misalign the offsets below; use a plain case-sensitive replace then.
    if lower.len() != text.len() {
        return text.replace(username, "<user>");
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&needle) {
        out.push_str(&text[i..i + pos]);
        out.push_str("<user>");
        i += pos + needle.len();
    }
    out.push_str(&text[i..]);
    out
}

fn csv_cell(s: &str) -> String {
    // A leading = + - @ makes spreadsheet programs treat the cell as a formula.
    let guarded = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
    if guarded.contains([',', '"', '\n', '\r']) { format!("\"{}\"", guarded.replace('"', "\"\"")) } else { guarded }
}

fn iso(secs: Option<i64>) -> String {
    secs.and_then(|s| chrono::DateTime::from_timestamp(s, 0)).map(|d| d.to_rfc3339()).unwrap_or_default()
}

/// Render a snapshot. With `redact`, the folder's full path is replaced by
/// `<folder>` and the account name is removed from every path.
pub fn export(snap: &Snapshot, format: Format, redact: bool, username: &str, exported_at: &str) -> Vec<u8> {
    let name = |s: &str| if redact { redact_user(s, username) } else { s.to_string() };
    match format {
        Format::Csv => {
            let mut out = String::from("path,type,size_bytes,modified_utc\r\n");
            for e in &snap.entries {
                out.push_str(&format!(
                    "{},{},{},{}\r\n",
                    csv_cell(&name(&e.path)),
                    if e.dir { "folder" } else { "file" },
                    e.size,
                    iso(e.modified)
                ));
            }
            out.into_bytes()
        }
        Format::Json => {
            let entries: Vec<serde_json::Value> = snap
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "path": name(&e.path), "type": if e.dir { "folder" } else { "file" },
                        "size_bytes": e.size, "modified_utc": iso(e.modified),
                    })
                })
                .collect();
            let doc = serde_json::json!({
                "app": "VoidRecoverPS5",
                "exported_at": exported_at,
                "redacted": redact,
                "snapshot": {
                    "label": name(&snap.label),
                    "note": name(&snap.note),
                    "kind": snap.kind,
                    "taken_at": snap.taken_at,
                    "root": if redact { "<folder>".to_string() } else { snap.root.clone() },
                    "file_count": snap.file_count,
                    "dir_count": snap.dir_count,
                    "total_bytes": snap.total_bytes,
                    "truncated": snap.truncated,
                    "skipped": snap.skipped,
                    "entries": entries,
                },
            });
            serde_json::to_vec_pretty(&doc).expect("json value serialises")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    fn snap(entries: Vec<Entry>) -> Snapshot {
        snapshot_from(
            "s".into(),
            "t".into(),
            "L".into(),
            "".into(),
            Kind::PcFolder,
            Path::new("C:\\Users\\alice\\Games"),
            ScanResult { entries, truncated: false, skipped: 0 },
        )
    }

    fn file(path: &str, size: u64, modified: i64) -> Entry {
        Entry { path: path.into(), dir: false, size, modified: Some(modified) }
    }

    #[test]
    fn scan_lists_files_and_folders_with_relative_paths() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "a.txt", b"12345");
        write(d.path(), "sub/b.bin", b"xyz");
        fs::create_dir_all(d.path().join("empty")).unwrap();
        let r = scan(d.path(), &no_cancel(), |_| {}).unwrap();
        let paths: Vec<&str> = r.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.txt", "empty", "sub", "sub/b.bin"]);
        let s = snapshot_from("i".into(), "t".into(), "x".into(), "".into(), Kind::PcFolder, d.path(), r);
        assert_eq!((s.file_count, s.dir_count, s.total_bytes), (2, 2, 8));
        assert!(!s.truncated);
    }

    #[test]
    fn scan_rejects_files_and_honours_cancel() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "a.txt", b"1");
        assert!(scan(&d.path().join("a.txt"), &no_cancel(), |_| {}).is_err());
        assert!(scan(&d.path().join("nope"), &no_cancel(), |_| {}).is_err());
        assert!(matches!(scan(d.path(), &AtomicBool::new(true), |_| {}), Err(ScanError::Cancelled)));
    }

    #[test]
    fn compare_finds_added_removed_changed_and_unchanged() {
        let old = snap(vec![file("keep.txt", 5, 100), file("gone.txt", 7, 100), file("edit.txt", 3, 100)]);
        let new = snap(vec![file("keep.txt", 5, 100), file("edit.txt", 4, 200), file("new.txt", 9, 300)]);
        let d = compare(&old, &new);
        assert_eq!(d.unchanged, 1);
        assert_eq!(d.added.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), vec!["new.txt"]);
        assert_eq!(d.removed.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), vec!["gone.txt"]);
        assert_eq!(d.changed.len(), 1);
        assert_eq!((d.changed[0].old_size, d.changed[0].new_size), (3, 4));
        assert!(d.moved.is_empty());
    }

    #[test]
    fn compare_pairs_unique_moves_and_ignores_ambiguous_ones() {
        let old = snap(vec![file("a/x.bin", 50, 10), file("a/p.bin", 8, 5), file("a/q.bin", 8, 5)]);
        let new = snap(vec![file("b/x.bin", 50, 10), file("b/p.bin", 8, 5), file("b/q.bin", 8, 5)]);
        let d = compare(&old, &new);
        assert_eq!(d.moved.len(), 1);
        assert_eq!((d.moved[0].from.as_str(), d.moved[0].to.as_str()), ("a/x.bin", "b/x.bin"));
        // The two identical 8-byte files stay as removed/added: it's a guess we won't make.
        assert_eq!(d.removed_total, 2);
        assert_eq!(d.added_total, 2);
    }

    #[test]
    fn compare_is_case_insensitive_on_paths() {
        let old = snap(vec![file("Photos/a.jpg", 5, 1)]);
        let new = snap(vec![file("photos/a.jpg", 5, 1)]);
        let d = compare(&old, &new);
        assert_eq!((d.added_total, d.removed_total, d.unchanged), (0, 0, 1));
    }

    #[test]
    fn redaction_removes_the_account_name_and_the_root() {
        assert_eq!(redact_user("C:\\Users\\Alice\\x\\alice.txt", "alice"), "C:\\Users\\<user>\\x\\<user>.txt");
        assert_eq!(redact_user("abc", "a"), "abc");
        let mut s = snap(vec![file("alice docs/a.txt", 1, 0)]);
        s.label = "Alice's stick".into();
        let json = String::from_utf8(export(&s, Format::Json, true, "alice", "now")).unwrap();
        assert!(!json.to_lowercase().contains("alice"), "{json}");
        assert!(json.contains("<folder>") && json.contains("<user> docs/a.txt"));
        let plain = String::from_utf8(export(&s, Format::Json, false, "alice", "now")).unwrap();
        assert!(plain.contains("C:\\\\Users\\\\alice\\\\Games"));
    }

    #[test]
    fn csv_quotes_and_defuses_formulas() {
        let s = snap(vec![file("=HYPERLINK(\"x\").txt", 1, 0), file("a,b.txt", 2, 0)]);
        let csv = String::from_utf8(export(&s, Format::Csv, false, "", "")).unwrap();
        assert!(csv.starts_with("path,type,size_bytes,modified_utc\r\n"));
        assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\").txt\",file,1,"));
        assert!(csv.contains("\"a,b.txt\",file,2,"));
    }

    #[test]
    fn notebook_round_trip_and_limits() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("notebook.json");
        let (mut nb, note) = Notebook::load(&path);
        assert!(note.is_none() && nb.snapshots.is_empty());
        nb.add(snap(vec![file("a", 1, 1)])).unwrap();
        nb.save(&path).unwrap();
        let (back, _) = Notebook::load(&path);
        assert_eq!(back.snapshots.len(), 1);
        fs::write(&path, b"not json").unwrap();
        assert!(Notebook::load(&path).1.is_some());
        for _ in 0..MAX_SNAPSHOTS {
            let _ = nb.add(snap(vec![]));
        }
        assert!(nb.add(snap(vec![])).is_err());
    }
}
