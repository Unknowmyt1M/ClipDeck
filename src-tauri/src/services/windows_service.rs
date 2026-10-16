//! Windows platform integration: startup registration + focus/paste.
//!
//! Startup uses the HKCU Run key (no admin required). The global hotkey is
//! registered via `tauri-plugin-global-shortcut` in `hotkey_service.rs`.
//! Auto-paste (quick picker) focuses the remembered foreground window and
//! sends Ctrl+V via `SendInput`.

use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HWND};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
    RegSetValueExW,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "ClipDeck";
/// `RegDeleteValueW` reports this when the value is already gone.
const ERROR_FILE_NOT_FOUND: u32 = 2;
/// ASCII 'V' — the second half of Ctrl+V.
const V_KEY: u16 = 0x56;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Run-key command line for an exe path.
///
/// Windows parses an unquoted Run value as "program + arguments", so
/// `C:\Program Files\ClipDeck\clipdeck.exe` would try to launch
/// `C:\Program` — autostart silently breaks for any path with spaces.
fn startup_command_line(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Enable/disable "start with Windows" via the per-user Run key.
pub fn set_startup_enabled(enabled: bool) -> Result<(), String> {
    // Resolve everything that can fail BEFORE opening the key: the old
    // flow's `?` on current_exe returned while the HKEY was still open.
    let value: Option<Vec<u16>> = if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
        Some(to_wide(&startup_command_line(&exe)))
    } else {
        None
    };

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

        let name = to_wide(VALUE_NAME);
        let result = match &value {
            Some(value) => {
                let ret = RegSetValueExW(
                    hkey,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast::<u8>(),
                    (value.len() * 2) as u32,
                );
                if ret == ERROR_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("RegSetValueExW failed: {ret}"))
                }
            }
            None => {
                let ret = RegDeleteValueW(hkey, name.as_ptr());
                // Missing value just means it was already disabled.
                if ret == ERROR_SUCCESS || ret == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(format!("RegDeleteValueW failed: {ret}"))
                }
            }
        };

        RegCloseKey(hkey);
        result
    }
}

// ---------- Auto-paste (quick picker) ----------

/// HWND of the current foreground window — unless it belongs to ClipDeck
/// itself (pasting a clip into our own window would be nonsense).
pub fn foreground_window() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == GetCurrentProcessId() {
            return None;
        }
        Some(hwnd as isize)
    }
}

/// Focus `hwnd` and send Ctrl+V into it.
///
/// Windows frequently refuses `SetForegroundWindow`, so focus is polled and
/// verified: keystrokes are only sent once the target *actually* owns the
/// foreground — firing blindly could paste the clip into the wrong window.
/// Error strings never contain clipboard content.
pub fn paste_into(hwnd: isize) -> Result<(), String> {
    let target = hwnd as HWND;
    unsafe {
        if IsWindow(target) == 0 {
            return Err("paste target window no longer exists".into());
        }
        if IsIconic(target) != 0 {
            ShowWindow(target, SW_RESTORE);
        }

        let mut focused = false;
        for _ in 0..20 {
            SetForegroundWindow(target);
            if GetForegroundWindow() == target {
                focused = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !focused {
            return Err("could not focus the paste target window".into());
        }

        // Brief settle so the target processes activation before the
        // keystrokes arrive (caret / input queue handoff).
        std::thread::sleep(std::time::Duration::from_millis(60));
        send_ctrl_v();
    }
    Ok(())
}

/// Synthesize Ctrl+V (Ctrl down, V down, V up, Ctrl up).
unsafe fn send_ctrl_v() {
    let key = |vk: u16, flags: u32| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        key(VK_CONTROL, 0),
        key(V_KEY, 0),
        key(V_KEY, KEYEVENTF_KEYUP),
        key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
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

    /// Unquoted Run values break autostart for any install path with
    /// spaces (e.g. `C:\Program Files\...`).
    #[test]
    fn startup_command_line_quotes_the_path() {
        let p = std::path::Path::new(r"C:\Program Files\ClipDeck\clipdeck.exe");
        assert_eq!(
            startup_command_line(p),
            r#""C:\Program Files\ClipDeck\clipdeck.exe""#
        );
    }
}
