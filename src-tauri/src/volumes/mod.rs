//! Drive discovery behind a small trait, so the rest of the app never touches
//! platform APIs and tests can use fake drives.
//!
//! The adapter reports facts; `blockers` turns them into a fail-closed
//! decision. A fact the platform could not establish is a blocker, not a guess.

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusKind {
    Usb,
    Other,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeInfo {
    /// Stable identity (Windows: Volume GUID path). Never a drive letter.
    pub id: String,
    /// Where the volume is mounted, e.g. `E:\`.
    pub mount_path: String,
    pub label: String,
    /// Upper-case file system name as the OS reports it (`FAT32`, `EXFAT`, `NTFS`).
    pub filesystem: String,
    /// Volume serial number; part of the identity re-check.
    pub serial: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub removable: bool,
    pub bus: BusKind,
    pub is_system: bool,
}

impl VolumeInfo {
    pub fn is_fat32(&self) -> bool {
        self.filesystem == "FAT32"
    }

    /// Everything that stops this volume from being a target. Empty = allowed.
    pub fn blockers(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.is_system {
            out.push("This is the drive Windows is running from.".to_string());
        }
        match self.filesystem.as_str() {
            "FAT32" | "EXFAT" => {}
            "" => out.push("The file system could not be read.".to_string()),
            other => out.push(format!(
                "The PS5 reads FAT32 or exFAT drives; this one is {other}. This app never formats drives; format it \
                 yourself with the operating system's tools (this erases the drive) and come back."
            )),
        }
        match self.bus {
            BusKind::Usb => {}
            BusKind::Other => out.push("This is not a USB drive.".to_string()),
            BusKind::Unknown => out.push("Could not confirm this is a USB drive.".to_string()),
        }
        if self.mount_path.is_empty() {
            out.push("The drive has no drive letter.".to_string());
        }
        out
    }
}

pub trait VolumeProvider: Send + Sync {
    fn list(&self) -> Result<Vec<VolumeInfo>, String>;

    /// Flush, lock, dismount and eject a volume so the drive can be unplugged.
    /// Fails with a plain message when the system refuses (files in use, no
    /// permission); the caller then points the user to the system's own eject.
    fn eject(&self, _volume: &VolumeInfo) -> Result<(), String> {
        Err("Ejecting from this app isn't available on this system.".into())
    }

    /// Re-enumerate and return the volume only if it is still the same one
    /// (same id, serial, file system and bus) and still allowed.
    fn recheck(&self, expected: &VolumeInfo) -> Result<VolumeInfo, String> {
        let now = self.list()?.into_iter().find(|v| v.id == expected.id).ok_or("The drive is no longer connected.")?;
        if now.serial != expected.serial || now.filesystem != expected.filesystem || now.bus != expected.bus {
            return Err("The drive changed since you chose it. Choose it again.".into());
        }
        let blockers = now.blockers();
        if let Some(first) = blockers.first() {
            return Err(first.clone());
        }
        Ok(now)
    }
}

pub fn system_provider() -> Box<dyn VolumeProvider> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsVolumes)
    }
    #[cfg(not(windows))]
    {
        Box::new(Unsupported)
    }
}

#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl VolumeProvider for Unsupported {
    fn list(&self) -> Result<Vec<VolumeInfo>, String> {
        Err("Building a USB drive is only available on Windows in this version.".into())
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::sync::Mutex;

    pub struct FakeVolumes(pub Mutex<Vec<VolumeInfo>>);

    impl VolumeProvider for FakeVolumes {
        fn list(&self) -> Result<Vec<VolumeInfo>, String> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    pub fn usb_stick(id: &str, mount: &str) -> VolumeInfo {
        VolumeInfo {
            id: id.into(),
            mount_path: mount.into(),
            label: "STICK".into(),
            filesystem: "EXFAT".into(),
            serial: 0xABCD_1234,
            total_bytes: 8 << 30,
            free_bytes: 7 << 30,
            removable: true,
            bus: BusKind::Usb,
            is_system: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn usb_exfat_and_fat32_are_allowed() {
        let mut v = usb_stick("a", "E:\\");
        assert!(v.blockers().is_empty());
        v.filesystem = "FAT32".into();
        assert!(v.blockers().is_empty());
    }

    #[test]
    fn fails_closed_on_anything_uncertain() {
        let mut v = usb_stick("a", "E:\\");
        v.filesystem = "NTFS".into();
        assert_eq!(v.blockers().len(), 1);
        v.filesystem = String::new();
        assert!(v.blockers()[0].contains("could not be read"));
        let mut v = usb_stick("a", "E:\\");
        v.bus = BusKind::Unknown;
        assert!(!v.blockers().is_empty());
        v.bus = BusKind::Other;
        assert!(!v.blockers().is_empty());
        let mut v = usb_stick("a", "E:\\");
        v.is_system = true;
        assert!(!v.blockers().is_empty());
        let mut v = usb_stick("a", "E:\\");
        v.mount_path.clear();
        assert!(!v.blockers().is_empty());
    }

    #[test]
    fn recheck_detects_swapped_or_removed_drive() {
        let chosen = usb_stick("vol1", "E:\\");
        let p = FakeVolumes(Mutex::new(vec![chosen.clone()]));
        assert!(p.recheck(&chosen).is_ok());

        // Same id, different serial: the stick was swapped.
        let mut swapped = chosen.clone();
        swapped.serial = 1;
        *p.0.lock().unwrap() = vec![swapped];
        assert!(p.recheck(&chosen).unwrap_err().contains("changed"));

        // Drive letter reused by another volume.
        *p.0.lock().unwrap() = vec![usb_stick("vol2", "E:\\")];
        assert!(p.recheck(&chosen).unwrap_err().contains("no longer connected"));

        // Same drive but reformatted to NTFS.
        let mut reformatted = chosen.clone();
        reformatted.filesystem = "NTFS".into();
        *p.0.lock().unwrap() = vec![reformatted];
        assert!(p.recheck(&chosen).is_err());
    }
}
