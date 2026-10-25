//! Local catalog cache in the app data folder. Mirror links fetched from
//! source pages live here, never in the repository.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::domain::catalog::OfficialLatest;
use crate::domain::types::{Mirror, PackageRecord, PackageType};
use crate::sources::SourceStatus;

pub const CACHE_SCHEMA_VERSION: u32 = 1;
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogCache {
    pub schema_version: u32,
    pub saved_at: Option<String>,
    pub statuses: Vec<SourceStatus>,
    pub records: Vec<PackageRecord>,
    pub official_latest: Option<OfficialLatest>,
}

impl CatalogCache {
    pub fn status(&self, source_id: &str) -> Option<&SourceStatus> {
        self.statuses.iter().find(|s| s.source_id == source_id)
    }

    pub fn mirror_by_url(&self, url: &str) -> Option<&Mirror> {
        self.records.iter().flat_map(|r| r.mirrors.iter()).find(|m| m.url == url)
    }

    /// Exact published size (from any source) for the package a URL points at.
    pub fn expected_exact_size_for_url(&self, url: &str) -> Option<u64> {
        let rec = self.records.iter().find(|r| r.mirrors.iter().any(|m| m.url == url))?;
        let same = |r: &&PackageRecord| {
            r.firmware_version == rec.firmware_version
                && r.package_type == rec.package_type
                && r.variant_label == rec.variant_label
                && matches!(r.package_type, PackageType::Update | PackageType::Reinstall)
        };
        self.records
            .iter()
            .filter(same)
            .find(|r| !r.size_is_approximate && r.published_size_bytes.is_some())
            .and_then(|r| r.published_size_bytes)
    }

    /// Load the cache; a missing, oversized or unreadable file gives an empty
    /// cache plus the reason, so the app still starts.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return (Self::default(), None),
        };
        if meta.len() > MAX_CACHE_BYTES {
            return (Self::default(), Some("The catalog cache was too large and was ignored.".into()));
        }
        match fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Self>(&t).map_err(|e| e.to_string()))
        {
            Ok(c) if c.schema_version == CACHE_SCHEMA_VERSION => (c, None),
            Ok(_) => (Self::default(), Some("The catalog cache is from another app version and was ignored.".into())),
            Err(e) => (Self::default(), Some(format!("The catalog cache could not be read and was ignored ({e})."))),
        }
    }

    /// Write via a temp file in the same folder, then rename over the old cache.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec(self).map_err(std::io::Error::other)?)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog-cache.json");
        let (empty, note) = CatalogCache::load(&path);
        assert!(empty.records.is_empty() && note.is_none());

        let c = CatalogCache { schema_version: CACHE_SCHEMA_VERSION, saved_at: Some("t".into()), ..Default::default() };
        c.save(&path).unwrap();
        assert_eq!(CatalogCache::load(&path).0.saved_at.as_deref(), Some("t"));

        fs::write(&path, "{not json").unwrap();
        let (c2, note) = CatalogCache::load(&path);
        assert!(c2.saved_at.is_none());
        assert!(note.unwrap().contains("could not be read"));
    }
}
