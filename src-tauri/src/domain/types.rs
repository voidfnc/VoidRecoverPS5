//! Core record types shared by the source adapters, the catalog and the UI.
//! Field names are serialized in snake_case and mirrored in `src/api/types.ts`.

use serde::{Deserialize, Serialize};

use super::version::FirmwareVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    OfficialPublisher,
    DeveloperRelease,
    CommunityArchive,
    CommunityReference,
    CommunityDirectory,
    CommunityDiscussion,
    UserAdded,
    LocalFileUnknown,
}

/// The app's normalized package category. Source labels are kept separately
/// in `PackageRecord::raw_package_label` / `source_section`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageType {
    Update,
    Reinstall,
    SpecialVariant,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    Md5,
    Sha256,
}

impl HashAlgorithm {
    pub fn hex_len(self) -> usize {
        match self {
            HashAlgorithm::Md5 => 32,
            HashAlgorithm::Sha256 => 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedHash {
    pub algorithm: HashAlgorithm,
    /// Lowercase hex.
    pub value: String,
    pub source_id: String,
    /// Set when this source copied the value from another registered source,
    /// so agreement between the two is not independent confirmation.
    pub derived_from: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMode {
    DirectHttps,
    ExternalBrowser,
    UserSuppliedLocalFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorScope {
    File,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable,
    BrowserOnly,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    /// User-facing host name, e.g. "Sony CDN", "MEGA".
    pub host: String,
    pub url: String,
    pub mode: MirrorMode,
    pub scope: MirrorScope,
    pub availability: Availability,
    pub probed_at: Option<String>,
    pub availability_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRecord {
    pub package_id: String,
    pub source_id: String,
    pub firmware_version: FirmwareVersion,
    /// Long build label when the source has one, e.g. "26.07-14.20.00.07-00.00.00.0.1".
    pub long_version: Option<String>,
    /// Sony build date as used in CDN paths, e.g. "2026_1014".
    pub build_date: Option<String>,
    pub source_section: Option<String>,
    pub raw_package_label: String,
    pub package_type: PackageType,
    pub variant_label: Option<String>,
    pub published_size_bytes: Option<u64>,
    pub size_is_approximate: bool,
    pub raw_size_text: Option<String>,
    pub hashes: Vec<PublishedHash>,
    pub mirrors: Vec<Mirror>,
    pub last_checked: String,
}

impl PackageRecord {
    pub fn hash(&self, algorithm: HashAlgorithm) -> Option<&PublishedHash> {
        self.hashes.iter().find(|h| h.algorithm == algorithm)
    }
}

/// Parse a size string as printed by the community sources:
/// "1266086912 B" (exact bytes) or "811MB" / "1.2 GB" (rounded).
/// Rounded values are converted with binary units and flagged approximate.
pub fn parse_size_text(text: &str) -> Option<(u64, bool)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase().replace(' ', "");
    if let Some(n) = lower.strip_suffix('b').filter(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty()) {
        return n.parse().ok().map(|v| (v, false));
    }
    let (num, mult) = if let Some(n) = lower.strip_suffix("gb") {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = lower.strip_suffix("mb") {
        (n, 1024u64 * 1024)
    } else if let Some(n) = lower.strip_suffix("kb") {
        (n, 1024u64)
    } else {
        return None;
    };
    let value: f64 = num.replace(',', ".").parse().ok()?;
    if !(value.is_finite() && value > 0.0) {
        return None;
    }
    Some(((value * mult as f64).round() as u64, true))
}

pub fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Normalize a hex digest from a source: trim, lowercase, and require the
/// exact length for the algorithm.
pub fn normalize_hex(value: &str, algorithm: HashAlgorithm) -> Option<String> {
    let v = value.trim().to_ascii_lowercase();
    is_lower_hex(&v, algorithm.hex_len()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_byte_sizes_are_not_approximate() {
        assert_eq!(parse_size_text("1266086912 B"), Some((1_266_086_912, false)));
    }

    #[test]
    fn rounded_sizes_are_approximate() {
        assert_eq!(parse_size_text("811MB"), Some((811 * 1024 * 1024, true)));
        assert_eq!(parse_size_text("1.5 GB").map(|(_, a)| a), Some(true));
    }

    #[test]
    fn blank_or_garbage_size_is_none() {
        assert_eq!(parse_size_text(""), None);
        assert_eq!(parse_size_text("big"), None);
        assert_eq!(parse_size_text("-3MB"), None);
    }

    #[test]
    fn hex_normalization_checks_length() {
        assert_eq!(
            normalize_hex(" F7853A0FA97A9A7828AC548798012782 ", HashAlgorithm::Md5).as_deref(),
            Some("f7853a0fa97a9a7828ac548798012782")
        );
        assert_eq!(normalize_hex("abc", HashAlgorithm::Md5), None);
        assert_eq!(normalize_hex(&"g".repeat(32), HashAlgorithm::Md5), None);
    }
}
