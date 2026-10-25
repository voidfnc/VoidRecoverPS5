//! Windows volume adapter: enumerates volumes by Volume GUID path and reads
//! serial, file system, drive type and USB bus type from the OS.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FindFirstVolumeW, FindNextVolumeW,
    FindVolumeClose, FlushFileBuffers, GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
    GetVolumePathNamesForVolumeNameW, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA, IOCTL_STORAGE_MEDIA_REMOVAL,
    IOCTL_STORAGE_QUERY_PROPERTY, PREVENT_MEDIA_REMOVAL, STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
};
use windows::core::PCWSTR;

use super::{BusKind, VolumeInfo, VolumeProvider};

const DRIVE_REMOVABLE: u32 = 2;
/// STORAGE_BUS_TYPE::BusTypeUsb
const BUS_TYPE_USB: i32 = 7;
const BUS_TYPE_UNKNOWN: i32 = 0;

pub struct WindowsVolumes;

const GENERIC_READ_WRITE: u32 = 0xC000_0000;

impl VolumeProvider for WindowsVolumes {
    fn list(&self) -> Result<Vec<VolumeInfo>, String> {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
        let mut out = Vec::new();
        let mut buf = [0u16; 260];
        // SAFETY: buffers are sized and outlive the calls; the handle is closed below.
        unsafe {
            let handle = FindFirstVolumeW(&mut buf).map_err(|e| format!("Could not list drives: {e}"))?;
            loop {
                let guid = from_wide(&buf);
                if let Some(v) = describe(&guid, &system_drive) {
                    out.push(v);
                }
                if FindNextVolumeW(handle, &mut buf).is_err() {
                    break;
                }
            }
            let _ = FindVolumeClose(handle);
        }
        Ok(out)
    }

    fn eject(&self, volume: &VolumeInfo) -> Result<(), String> {
        eject_volume(volume)
    }
}

/// Eject by the documented sequence: flush, lock, dismount, allow removal, eject.
/// Locking fails while any program still has a file open on the drive.
fn eject_volume(volume: &VolumeInfo) -> Result<(), String> {
    let letter = volume.mount_path.trim_end_matches('\\');
    if letter.len() != 2 || !letter.ends_with(':') {
        return Err("The drive has no plain drive letter.".into());
    }
    let device = wide(&format!("\\\\.\\{letter}"));
    // SAFETY: `device` is NUL-terminated; the handle is closed on every path below.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device.as_ptr()),
            GENERIC_READ_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|_| "Windows would not let this app open the drive for ejecting.".to_string())?;
    let result = unsafe { eject_steps(handle) };
    // SAFETY: handle came from CreateFileW above.
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

unsafe fn eject_steps(handle: HANDLE) -> Result<(), String> {
    let mut returned = 0u32;
    // SAFETY (all calls): handle is valid and buffers outlive each call.
    unsafe {
        FlushFileBuffers(handle).map_err(|_| "Windows could not flush the drive.".to_string())?;
        let mut locked = false;
        for _ in 0..5 {
            if DeviceIoControl(handle, FSCTL_LOCK_VOLUME, None, 0, None, 0, Some(&mut returned), None).is_ok() {
                locked = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        if !locked {
            return Err("Another program is still using the drive, so it can't be ejected yet.".into());
        }
        DeviceIoControl(handle, FSCTL_DISMOUNT_VOLUME, None, 0, None, 0, Some(&mut returned), None)
            .map_err(|_| "Windows could not dismount the drive.".to_string())?;
        let allow = PREVENT_MEDIA_REMOVAL { PreventMediaRemoval: false };
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_MEDIA_REMOVAL,
            Some(&allow as *const _ as *const c_void),
            std::mem::size_of::<PREVENT_MEDIA_REMOVAL>() as u32,
            None,
            0,
            Some(&mut returned),
            None,
        )
        .map_err(|_| "Windows could not unlock the drive for removal.".to_string())?;
        DeviceIoControl(handle, IOCTL_STORAGE_EJECT_MEDIA, None, 0, None, 0, Some(&mut returned), None)
            .map_err(|_| "Windows could not eject the drive.".to_string())?;
    }
    Ok(())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Facts about one volume. Volumes with no drive letter, or whose basic
/// information can't be read (empty card readers, locked volumes), are skipped
/// because they could not be written to anyway.
unsafe fn describe(guid_path: &str, system_drive: &str) -> Option<VolumeInfo> {
    let guid_w = wide(guid_path);
    let mut names = vec![0u16; 1024];
    let mut len = 0u32;
    // SAFETY: `guid_w` is NUL-terminated and `names` is a valid buffer.
    unsafe { GetVolumePathNamesForVolumeNameW(PCWSTR(guid_w.as_ptr()), Some(&mut names), &mut len).ok()? };
    let mount = from_wide(&names);
    if mount.is_empty() {
        return None;
    }
    let mount_w = wide(&mount);

    let mut label = [0u16; 261];
    let mut fs = [0u16; 64];
    let mut serial = 0u32;
    // SAFETY: all pointers refer to live locals.
    unsafe {
        GetVolumeInformationW(PCWSTR(mount_w.as_ptr()), Some(&mut label), Some(&mut serial), None, None, Some(&mut fs))
            .ok()?
    };

    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: as above.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(mount_w.as_ptr()), Some(&mut free), Some(&mut total), None).ok()? };

    // SAFETY: `mount_w` is NUL-terminated.
    let removable = unsafe { GetDriveTypeW(PCWSTR(mount_w.as_ptr())) } == DRIVE_REMOVABLE;
    let is_system = mount.to_uppercase().starts_with(system_drive);
    Some(VolumeInfo {
        id: guid_path.to_string(),
        mount_path: mount,
        label: from_wide(&label),
        filesystem: from_wide(&fs).to_uppercase(),
        serial,
        total_bytes: total,
        free_bytes: free,
        removable,
        bus: unsafe { bus_kind(guid_path) },
        is_system,
    })
}

/// Ask the storage stack for the bus type behind a volume. Any failure is
/// `Unknown`, which blocks the volume.
unsafe fn bus_kind(guid_path: &str) -> BusKind {
    let device = wide(guid_path.trim_end_matches('\\'));
    // Zero access: query only, never read or write the device.
    // SAFETY: `device` is NUL-terminated.
    let handle = match unsafe {
        CreateFileW(
            PCWSTR(device.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return BusKind::Unknown,
    };
    let kind = unsafe { query_bus(handle) };
    // SAFETY: handle came from CreateFileW above.
    unsafe {
        let _ = CloseHandle(handle);
    }
    kind
}

unsafe fn query_bus(handle: HANDLE) -> BusKind {
    // Zeroed query = StorageDeviceProperty + PropertyStandardQuery.
    // SAFETY: STORAGE_PROPERTY_QUERY is plain data; all-zero is valid.
    let query: STORAGE_PROPERTY_QUERY = unsafe { std::mem::zeroed() };
    // u32-backed so the descriptor is properly aligned.
    let mut out = vec![0u32; 256];
    let mut returned = 0u32;
    // SAFETY: buffers are valid for the sizes passed.
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const c_void),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(out.as_mut_ptr() as *mut c_void),
            (out.len() * 4) as u32,
            Some(&mut returned),
            None,
        )
    };
    if ok.is_err() || (returned as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
        return BusKind::Unknown;
    }
    // SAFETY: the OS filled at least a full descriptor into an aligned buffer.
    let desc = unsafe { &*(out.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
    match desc.BusType.0 {
        BUS_TYPE_USB => BusKind::Usb,
        BUS_TYPE_UNKNOWN => BusKind::Unknown,
        _ => BusKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_without_panicking_and_system_drive_is_blocked() {
        let vols = WindowsVolumes.list().expect("volume enumeration works on a Windows host");
        eprintln!("{vols:#?}");
        for v in vols.iter().filter(|v| v.is_system) {
            assert!(!v.blockers().is_empty());
        }
        // No volume may be allowed without a positive USB + file system answer.
        for v in vols.iter().filter(|v| v.blockers().is_empty()) {
            assert_eq!(v.bus, BusKind::Usb);
            assert!(matches!(v.filesystem.as_str(), "FAT32" | "EXFAT"));
        }
    }
}
