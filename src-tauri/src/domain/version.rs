//! PS5 system software version numbers ("14.20.00").

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl FirmwareVersion {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}.{:02}.{:02}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a firmware version: {0:?}")]
pub struct VersionParseError(pub String);

impl FromStr for FirmwareVersion {
    type Err = VersionParseError;

    /// Accepts "14.20.00", "14.20" (patch 00) and "4.03" style input.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || VersionParseError(s.to_string());
        let parts: Vec<&str> = s.trim().split('.').collect();
        if !(2..=3).contains(&parts.len()) {
            return Err(err());
        }
        let num = |p: &str| -> Result<u8, VersionParseError> {
            if p.is_empty() || p.len() > 2 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err(err());
            }
            p.parse().map_err(|_| err())
        };
        let major = num(parts[0])?;
        let minor = num(parts[1])?;
        let patch = if parts.len() == 3 { num(parts[2])? } else { 0 };
        Ok(Self { major, minor, patch })
    }
}

impl Serialize for FirmwareVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FirmwareVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let v: FirmwareVersion = "14.20.00".parse().unwrap();
        assert_eq!(v, FirmwareVersion::new(14, 20, 0));
        assert_eq!(v.to_string(), "14.20.00");
        assert_eq!("4.03".parse::<FirmwareVersion>().unwrap().to_string(), "04.03.00");
    }

    #[test]
    fn orders_numerically() {
        let a: FirmwareVersion = "9.60.00".parse().unwrap();
        let b: FirmwareVersion = "10.01.00".parse().unwrap();
        assert!(a < b);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "14", "14.20.00.07", "a.b.c", "14.200.00", "14..00"] {
            assert!(bad.parse::<FirmwareVersion>().is_err(), "{bad}");
        }
    }
}
