//! Windows platform integration: startup registration.
//!
//! Startup uses the HKCU Run key (no admin required). The global hotkey is
//! registered via `tauri-plugin-global-shortcut` in `hotkey_service.rs`.

use std::ptr;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
    RegSetValueExW,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "ClipDeck";
/// `RegDeleteValueW` reports this when the value is already gone.
const ERROR_FILE_NOT_FOUND: u32 = 2;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Enable/disable "start with Windows" via the per-user Run key.
pub fn set_startup_enabled(enabled: bool) -> Result<(), String> {
    unsafe {
        let key_name = to_wide(RUN_KEY);
        let mut hkey = ptr::null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_name.as_ptr(),
            0,
            ptr::null(),
            0,
            KEY_WRITE,
            ptr::null(),
            &mut hkey,
            ptr::null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW failed: {status}"));
        }

        let result = if enabled {
            let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
            let mut value: Vec<u16> = exe
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let name = to_wide(VALUE_NAME);
            let ret = RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_mut_ptr().cast::<u8>().cast_const(),
                (value.len() * 2) as u32,
            );
            if ret != ERROR_SUCCESS {
                Err(format!("RegSetValueExW failed: {ret}"))
            } else {
                Ok(())
            }
        } else {
            let name = to_wide(VALUE_NAME);
            let ret = RegDeleteValueW(hkey, name.as_ptr());
            // Missing value just means it was already disabled.
            if ret == ERROR_SUCCESS || ret == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("RegDeleteValueW failed: {ret}"))
            }
        };

        RegCloseKey(hkey);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_conversion_terminates() {
        let w = to_wide("abc");
        assert_eq!(w, vec![97, 98, 99, 0]);
    }
}
