//! The bundled source registry (`manifests/sources.json`).
//!
//! Source records are data only. Loading fails closed on anything that looks
//! executable, on unknown fields, on non-https URLs and on unknown adapters.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::SourceType;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_DEPTH: usize = 8;
/// Adapters compiled into the app. A manifest can only refer to these.
pub const KNOWN_ADAPTERS: &[&str] = &["mewx_index", "sony_cdn", "darthsternie_ps5"];
const FORBIDDEN_KEYS: &[&str] = &[
    "exec",
    "command",
    "commands",
    "cmd",
    "script",
    "scripts",
    "shell",
    "args",
    "hook",
    "hooks",
    "post_download",
    "run",
    "eval",
    "plugin",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcesManifest {
    pub schema_version: u32,
    pub catalog_version: String,
    pub generated_at: String,
    pub sources: Vec<SourceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub source_id: String,
    pub name: String,
    pub source_type: SourceType,
    pub source_page: String,
    pub maintainer: Option<String>,
    pub last_checked: String,
    pub adapter: Option<String>,
    #[serde(default)]
    pub derived_from: Vec<String>,
    pub notes: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest is larger than {MAX_MANIFEST_BYTES} bytes")]
    TooLarge,
    #[error("manifest is not valid JSON: {0}")]
    Json(String),
    #[error("manifest nests deeper than {MAX_DEPTH} levels")]
    TooDeep,
    #[error("manifest contains a forbidden field {0:?}; source records are data only")]
    ForbiddenField(String),
    #[error("unsupported schema_version {0}")]
    UnsupportedSchema(u32),
    #[error("source {source_id}: {problem}")]
    InvalidSource { source_id: String, problem: String },
    #[error("duplicate source_id {0:?}")]
    DuplicateId(String),
}

impl SourcesManifest {
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if text.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge);
        }
        let raw: Value = serde_json::from_str(text).map_err(|e| ManifestError::Json(e.to_string()))?;
        scan_value(&raw, 0)?;
        let manifest: SourcesManifest = serde_json::from_value(raw).map_err(|e| ManifestError::Json(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema(self.schema_version));
        }
        let ids: HashSet<&str> = self.sources.iter().map(|s| s.source_id.as_str()).collect();
        let mut seen = HashSet::new();
        for s in &self.sources {
            let invalid = |problem: &str| ManifestError::InvalidSource {
                source_id: s.source_id.clone(),
                problem: problem.to_string(),
            };
            if !seen.insert(s.source_id.as_str()) {
                return Err(ManifestError::DuplicateId(s.source_id.clone()));
            }
            if s.source_id.is_empty()
                || !s.source_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(invalid("source_id must be lowercase letters, digits and '-'"));
            }
            if !is_https_url(&s.source_page) {
                return Err(invalid("source_page must be an https URL"));
            }
            if let Some(adapter) = &s.adapter
                && !KNOWN_ADAPTERS.contains(&adapter.as_str())
            {
                return Err(invalid("unknown adapter"));
            }
            for d in &s.derived_from {
                if !ids.contains(d.as_str()) || d == &s.source_id {
                    return Err(invalid("derived_from must name another registered source"));
                }
            }
        }
        Ok(())
    }

    pub fn source(&self, id: &str) -> Option<&SourceRecord> {
        self.sources.iter().find(|s| s.source_id == id)
    }
}

/// Recursively reject forbidden keys and excessive nesting before typed parsing,
/// so the error names the problem instead of a generic "unknown field".
fn scan_value(v: &Value, depth: usize) -> Result<(), ManifestError> {
    if depth > MAX_DEPTH {
        return Err(ManifestError::TooDeep);
    }
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                let key = k.to_ascii_lowercase();
                if FORBIDDEN_KEYS.contains(&key.as_str()) {
                    return Err(ManifestError::ForbiddenField(k.clone()));
                }
                scan_value(child, depth + 1)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                scan_value(child, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn is_https_url(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && !host.contains('@') && !s.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = include_str!("../../../manifests/sources.json");

    fn minimal(extra_field: &str) -> String {
        format!(
            r#"{{"schema_version":1,"catalog_version":"0","generated_at":"x","sources":[
              {{"source_id":"a","name":"A","source_type":"community_archive",
                "source_page":"https://example.org/","maintainer":null,"last_checked":"x",
                "adapter":null,"notes":""{extra_field}}}]}}"#
        )
    }

    #[test]
    fn bundled_manifest_is_valid() {
        let m = SourcesManifest::parse(BUNDLED).expect("bundled manifest must validate");
        for id in ["mewx-ps5-fw-list", "sony-ps5-cdn", "darthsternie-ps5"] {
            assert!(m.source(id).is_some(), "{id} missing");
        }
    }

    #[test]
    fn rejects_executable_fields_anywhere() {
        for field in [r#","command":"rm -rf /""#, r#","Script":"x""#, r#","hooks":[]"#] {
            let err = SourcesManifest::parse(&minimal(field)).unwrap_err();
            assert!(matches!(err, ManifestError::ForbiddenField(_)), "{field}: {err:?}");
        }
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(matches!(SourcesManifest::parse(&minimal(r#","surprise":1"#)), Err(ManifestError::Json(_))));
    }

    #[test]
    fn rejects_non_https_and_unknown_adapter() {
        let http = minimal("").replace("https://example.org/", "http://example.org/");
        assert!(matches!(SourcesManifest::parse(&http), Err(ManifestError::InvalidSource { .. })));
        let adapter = minimal("").replace(r#""adapter":null"#, r#""adapter":"evil_adapter""#);
        assert!(matches!(SourcesManifest::parse(&adapter), Err(ManifestError::InvalidSource { .. })));
    }

    #[test]
    fn rejects_deep_nesting_and_oversize() {
        let deep = format!("{}1{}", "[".repeat(20), "]".repeat(20));
        assert_eq!(SourcesManifest::parse(&deep).unwrap_err(), ManifestError::TooDeep);
        let big = " ".repeat(MAX_MANIFEST_BYTES + 1);
        assert_eq!(SourcesManifest::parse(&big).unwrap_err(), ManifestError::TooLarge);
    }

    #[test]
    fn https_url_check() {
        assert!(is_https_url("https://darthsternie.net/ps5-firmwares/"));
        for bad in ["http://x.org", "https://", "https://user@evil.org/", "javascript:alert(1)", "https://a b"] {
            assert!(!is_https_url(bad), "{bad}");
        }
    }
}
