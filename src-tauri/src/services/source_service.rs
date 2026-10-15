//! Source detection — which application produced the clipboard content.
//!
//! Uses the Windows foreground window at capture time: window title +
//! executable name. Best-effort; failures return `None` and never block
//! capture.

use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

/// What we know about the source application.
#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    pub application_name: String,
    pub application_executable: String,
    pub window_title: String,
}

/// Capture the foreground window's identity.
///
/// # Safety
/// Calls raw Win32 APIs; all handles are closed before returning.
pub fn detect_source() -> Option<SourceInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // Window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let window_title = if title_len > 0 {
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        } else {
            String::new()
        };

        // Process id → executable path
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return Some(SourceInfo {
                window_title,
                ..Default::default()
            });
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Some(SourceInfo {
                window_title,
                ..Default::default()
            });
        }

        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            path_buf.as_mut_ptr(),
            &mut size,
        );
        windows_sys::Win32::Foundation::CloseHandle(handle);

        if ok == 0 {
            return Some(SourceInfo {
                window_title,
                ..Default::default()
            });
        }

        let path = String::from_utf16_lossy(&path_buf[..size as usize]);
        let exe = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // Friendly name without extension: "chrome.exe" → "chrome"
        let app_name = exe
            .strip_suffix(".exe")
            .map(|s| s.to_string())
            .unwrap_or_else(|| exe.clone());

        Some(SourceInfo {
            application_name: app_name,
            application_executable: exe,
            window_title,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_source_never_panics() {
        // In headless/CI contexts this may return None — that's fine.
        let _ = detect_source();
    }
}
