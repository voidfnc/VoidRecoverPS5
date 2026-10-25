//! Direct download of a package from Sony's server into the app's own
//! download folder. The address always comes from the catalog, never from the
//! UI. https is enforced on every redirect hop by the client; partial files
//! are removed on failure or cancel.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::{Stream, StreamExt};
use md5::Md5;
use sha2::{Digest, Sha256};

use crate::integrity::LocalHashes;

const PROGRESS_STEP: u64 = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Download cancelled.")]
    Cancelled,
    #[error("Sony's server answered HTTP {0}, so the file is not available from there right now.")]
    Status(u16),
    #[error("Sony's server reports {reported} bytes but the catalog expects {expected}. Nothing was saved.")]
    SizeMismatch { reported: u64, expected: u64 },
    #[error("The connection ended after {got} of {expected} bytes. Nothing was saved.")]
    Truncated { got: u64, expected: u64 },
    #[error("Network error: {0}")]
    Network(String),
    #[error("Could not save the file: {0}")]
    Io(String),
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Io(e.to_string())
    }
}

/// Download `url` to `dest_dir/final_name`, hashing as it streams.
/// `expected_size` is the size the catalog states exactly, if it does.
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    dest_dir: &Path,
    final_name: &str,
    expected_size: Option<u64>,
    cancel: &AtomicBool,
    progress: impl FnMut(u64, u64),
) -> Result<(PathBuf, LocalHashes), DownloadError> {
    let resp = client.get(url).send().await.map_err(|e| DownloadError::Network(e.to_string()))?;
    if resp.status() != reqwest::StatusCode::OK {
        return Err(DownloadError::Status(resp.status().as_u16()));
    }
    let reported = resp.content_length();
    if let (Some(reported), Some(expected)) = (reported, expected_size)
        && reported != expected
    {
        return Err(DownloadError::SizeMismatch { reported, expected });
    }
    let stream = resp.bytes_stream().map(|c| c.map_err(|e| e.to_string()));
    save_stream(stream, dest_dir, final_name, reported.or(expected_size), cancel, progress).await
}

/// The part of `download` that doesn't need a network, so it can be tested.
pub async fn save_stream<B: AsRef<[u8]>>(
    mut stream: impl Stream<Item = Result<B, String>> + Unpin,
    dest_dir: &Path,
    final_name: &str,
    expected_len: Option<u64>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(PathBuf, LocalHashes), DownloadError> {
    fs::create_dir_all(dest_dir)?;
    let part = dest_dir.join(format!("{final_name}.part"));
    let fin = dest_dir.join(final_name);
    let result = write_parts(&mut stream, &part, expected_len, cancel, &mut progress).await;
    match result {
        Ok(local) => {
            let moved = (|| -> std::io::Result<()> {
                if fin.exists() {
                    fs::remove_file(&fin)?;
                }
                fs::rename(&part, &fin)
            })();
            if let Err(e) = moved {
                let _ = fs::remove_file(&part);
                return Err(e.into());
            }
            Ok((fin, local))
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

async fn write_parts<B: AsRef<[u8]>>(
    stream: &mut (impl Stream<Item = Result<B, String>> + Unpin),
    part: &Path,
    expected_len: Option<u64>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<LocalHashes, DownloadError> {
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(part)?;
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut done = 0u64;
    let mut next_report = PROGRESS_STEP;
    let total = expected_len.unwrap_or(0);
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        let chunk = chunk.map_err(DownloadError::Network)?;
        let bytes = chunk.as_ref();
        sha.update(bytes);
        md5.update(bytes);
        file.write_all(bytes)?;
        done += bytes.len() as u64;
        if let Some(expected) = expected_len
            && done > expected
        {
            return Err(DownloadError::SizeMismatch { reported: done, expected });
        }
        if done >= next_report {
            progress(done, total);
            next_report = done + PROGRESS_STEP;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(DownloadError::Cancelled);
    }
    if let Some(expected) = expected_len
        && done != expected
    {
        return Err(DownloadError::Truncated { got: done, expected });
    }
    file.flush()?;
    file.sync_all()?;
    progress(done, total.max(done));
    Ok(LocalHashes { size_bytes: done, sha256: hex::encode(sha.finalize()), md5: hex::encode(md5.finalize()) })
}

/// Remove `.part` files an interrupted download left behind.
pub fn clean_partials(dest_dir: &Path) {
    let Ok(read) = fs::read_dir(dest_dir) else { return };
    for e in read.flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".pup.part") {
            let _ = fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    fn chunks(parts: &[&[u8]]) -> impl Stream<Item = Result<Vec<u8>, String>> + Unpin {
        stream::iter(parts.iter().map(|p| Ok(p.to_vec())).collect::<Vec<_>>())
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[tokio::test]
    async fn saves_and_hashes_the_stream() {
        let dir = tempfile::tempdir().unwrap();
        let (path, local) = save_stream(
            chunks(&[b"hello ", b"world"]),
            dir.path(),
            "a.PUP",
            Some(11),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello world");
        assert_eq!(local.size_bytes, 11);
        assert_eq!(local.sha256, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
        assert_eq!(files(dir.path()), vec!["a.PUP".to_string()]);
    }

    #[tokio::test]
    async fn truncated_download_leaves_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = save_stream(chunks(&[b"abc"]), dir.path(), "a.PUP", Some(10), &AtomicBool::new(false), |_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(err, DownloadError::Truncated { got: 3, expected: 10 }));
        assert!(files(dir.path()).is_empty());
    }

    #[tokio::test]
    async fn oversized_download_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let err = save_stream(chunks(&[b"abcdef"]), dir.path(), "a.PUP", Some(3), &AtomicBool::new(false), |_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(err, DownloadError::SizeMismatch { .. }));
        assert!(files(dir.path()).is_empty());
    }

    #[tokio::test]
    async fn cancel_and_network_errors_clean_up_and_keep_an_older_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.PUP"), b"older good copy").unwrap();
        let err = save_stream(chunks(&[b"abc"]), dir.path(), "a.PUP", None, &AtomicBool::new(true), |_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(err, DownloadError::Cancelled));
        let broken = stream::iter(vec![Ok(b"abc".to_vec()), Err("reset".to_string())]);
        let err = save_stream(broken, dir.path(), "a.PUP", None, &AtomicBool::new(false), |_, _| {}).await.unwrap_err();
        assert!(matches!(err, DownloadError::Network(_)));
        assert_eq!(files(dir.path()), vec!["a.PUP".to_string()]);
        assert_eq!(fs::read(dir.path().join("a.PUP")).unwrap(), b"older good copy");
    }

    #[test]
    fn partials_are_cleaned_but_finished_files_stay() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.PUP.part"), b"1").unwrap();
        fs::write(dir.path().join("x.PUP"), b"2").unwrap();
        clean_partials(dir.path());
        assert_eq!(files(dir.path()), vec!["x.PUP".to_string()]);
    }
}
