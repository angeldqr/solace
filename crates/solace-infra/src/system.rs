/// Whether the OS shows notification toasts. On Windows a user can switch them off for every app
/// (Settings > System > Notifications); toasts are then dropped without any error.
pub fn toasts_enabled() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW,
        };
        let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\PushNotifications\0"
            .encode_utf16()
            .collect();
        let name: Vec<u16> = "ToastEnabled\0".encode_utf16().collect();
        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        // SAFETY: valid NUL-terminated strings and a u32 buffer of the size we pass.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        // No value means the default, which is on.
        status != 0 || value != 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Whether the window with this handle is the one the user is using (foreground, not minimized).
/// Tauri's `is_focused` can't tell: keyboard focus sits in the WebView child window.
#[cfg(windows)]
pub fn is_foreground(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic};
    // SAFETY: plain queries on a window handle; a stale handle just compares unequal.
    unsafe {
        let window = hwnd as windows_sys::Win32::Foundation::HWND;
        GetForegroundWindow() == window && IsIconic(window) == 0
    }
}
