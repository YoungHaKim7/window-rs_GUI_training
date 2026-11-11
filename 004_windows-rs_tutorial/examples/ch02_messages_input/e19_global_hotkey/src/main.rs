//! e19 — Global Hotkey: Ctrl+Alt+H works from ANY window
//!
//! What you learn:
//! * `RegisterHotKey` — a system-wide key combination delivered to *your*
//!   window as `WM_HOTKEY`, even when your app has no focus at all
//! * Hotkey ids are per-window, and registration can FAIL (another app
//!   already owns the combo) — always handle that
//! * `SetForegroundWindow` — the polite way to steal focus in response
//! * `MOD_NOREPEAT` — no auto-repeat spam while the user holds the combo
//!
//! Run this, then switch to some other app and press Ctrl+Alt+H.
//!
//! New Win32 APIs: `RegisterHotKey`, `UnregisterHotKey`, `WM_HOTKEY`,
//! `SetForegroundWindow`, `MessageBeep`.
//!
//! Run: `cargo run -p e19_global_hotkey`

use windows::{Win32::*, core::*};

const HOTKEY_BRING_UP: i32 = 1; // our id; comes back in wparam of WM_HOTKEY

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e19_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e19_class"),
            w!("e19 · Global Hotkey"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 560, 200,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // (MOD_CONTROL | MOD_ALT | MOD_NOREPEAT) as u32 — MOD_* are i32.
        // 'H' as u32 — for letters/digits the VK code equals the ASCII
        // value, which is what RegisterHotKey wants.
        let registered = RegisterHotKey(
            Some(hwnd),
            HOTKEY_BRING_UP,
            (MOD_CONTROL | MOD_ALT | MOD_NOREPEAT) as u32,
            'H' as u32,
        );
        if !registered.as_bool() {
            // Hotkeys are a system-wide namespace: someone else may already
            // have claimed Ctrl+Alt+H. A real app would offer alternatives;
            // we explain and carry on without the feature.
            MessageBoxW(
                None,
                w!("RegisterHotKey failed — another program owns Ctrl+Alt+H.\nThe window still runs; the hotkey just won't fire."),
                w!("e19"),
                (MB_OK | MB_ICONEXCLAMATION) as u32,
            );
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_HOTKEY => {
                // wparam = the id you registered. lparam packs the
                // modifiers (low word) and the vk (high word) if you want
                // to double-check which combo fired.
                if wparam as i32 == HOTKEY_BRING_UP {
                    // Restore (if minimized) and come to the front.
                    _ = ShowWindow(hwnd, SW_RESTORE);
                    _ = SetForegroundWindow(hwnd);

                    // A little feedback + a timestamped title.
                    _ = MessageBeep(MB_ICONASTERISK as u32);
                    let mut now = SYSTEMTIME::default();
                    GetLocalTime(&mut now);
                    let title = HSTRING::from(format!(
                        "e19 · hotkey fired at {:02}:{:02}:{:02}",
                        now.wHour, now.wMinute, now.wSecond
                    ));
                    _ = SetWindowTextW(hwnd, &title);
                }
                0
            }
            WM_DESTROY => {
                // Good hygiene even though Windows cleans up on exit:
                // the id becomes available for other apps again.
                _ = UnregisterHotKey(Some(hwnd), HOTKEY_BRING_UP);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
