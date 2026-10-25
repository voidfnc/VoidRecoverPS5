//! The two language hints the window needs for its *Automatic* setting: the
//! language chosen in the installer (the NSIS installer records it in the
//! registry) and the Windows display language. Both are Windows language ids;
//! the window maps them to its own language codes.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SystemLanguages {
    /// Language id picked in the installer, if it was run and left a record.
    pub installer: Option<u32>,
    /// The Windows display language id.
    pub windows_ui: Option<u32>,
}

/// The installer stores the id as decimal text (for example `1049`).
pub fn parse_lcid(text: &str) -> Option<u32> {
    let t = text.trim();
    if t.is_empty() || t.len() > 6 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok().filter(|&n| n > 0)
}

pub fn read() -> SystemLanguages {
    SystemLanguages { installer: installer_lcid(), windows_ui: windows_ui_lcid() }
}

#[cfg(windows)]
fn windows_ui_lcid() -> Option<u32> {
    // SAFETY: no arguments, returns a plain number.
    let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    (id != 0).then_some(u32::from(id))
}

#[cfg(windows)]
fn installer_lcid() -> Option<u32> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;

    let mut buf = [0u16; 16];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its size outlive the call; the names are static, NUL-terminated.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\voidfnc\\VoidRecoverPS5"),
            w!("Installer Language"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if status.0 != 0 {
        return None;
    }
    let len = (bytes as usize / 2).saturating_sub(1).min(buf.len());
    parse_lcid(&String::from_utf16_lossy(&buf[..len]))
}

#[cfg(not(windows))]
fn windows_ui_lcid() -> Option<u32> {
    None
}

#[cfg(not(windows))]
fn installer_lcid() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installer_ids_strictly() {
        assert_eq!(parse_lcid("1049"), Some(1049));
        assert_eq!(parse_lcid(" 1033 "), Some(1033));
        assert_eq!(parse_lcid(""), None);
        assert_eq!(parse_lcid("0"), None);
        assert_eq!(parse_lcid("12a"), None);
        assert_eq!(parse_lcid("-5"), None);
        assert_eq!(parse_lcid("1234567"), None);
    }

    #[test]
    fn reading_never_panics() {
        let _ = read();
    }
}
