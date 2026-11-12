//! e93 — Registry Settings: a window that remembers where you put it
//!
//! What you learn:
//! * The registry read/write cycle with the raw API: `RegCreateKeyExW`
//!   opens (or creates) `HKCU\Software\windows-rs-tutorial\e93`, values go
//!   in with `RegSetValueExW` and come out with `RegQueryValueExW`, and the
//!   key handle is an OS resource you `RegCloseKey`
//! * REG_DWORD values — the simplest payload: four bytes in, four bytes
//!   out, cast through `*const u8`/`*mut u8` because the API is byte-typed
//! * "First run" detection the honest way: `RegQueryValueExW` returns
//!   `ERROR_FILE_NOT_FOUND` when the value doesn't exist yet — treat that
//!   as "use defaults", anything else as data
//! * Saving UI state at `WM_DESTROY` (the window still exists, so
//!   `GetWindowRect` still works) — the classic place settings apps persist
//!   window placement
//!
//! Move and resize the window, close it, run again: it reappears where you
//! left it. (Note: the vendored repo also ships a friendlier `windows-registry`
//! crate with typed get/set helpers; this example uses the raw Win32 API
//! because seeing the handles and status codes once is the lesson.)
//!
//! New Win32 APIs: `RegCreateKeyExW`, `RegQueryValueExW`, `RegSetValueExW`,
//! `RegCloseKey`, `HKEY_CURRENT_USER`, `REG_DWORD` (winreg/winnt);
//! `GetWindowRect`, `IsIconic` (winuser).
//!
//! Run: `cargo run -p e93_registry_settings`

use windows::{Win32::*, core::*};

/// Where our four DWORDs live. RegCreateKeyExW creates every missing level
/// of this path on first write — no need to pre-create anything.
const SUBKEY: PCWSTR = w!("Software\\windows-rs-tutorial\\e93");

#[derive(Default)]
struct State {
    /// The open key handle. Kept for the window's whole life so WM_DESTROY
    /// can write without reopening. HKEY is a raw handle: never close it
    /// twice, and only close it when we're done for good.
    hkey: HKEY,
    /// True when the values were found on startup — only used for the
    /// WM_PAINT caption.
    restored: bool,
}

fn main() {
    unsafe {
        // ---- 1. Open (or create) our key and read the saved placement ----
        let mut hkey = HKEY::default();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            SUBKEY,
            None,                           // reserved, must be None
            PCWSTR::null(),                 // no class string
            REG_OPTION_NON_VOLATILE as u32, // a normal (on-disk) key
            (KEY_READ | KEY_WRITE) as REGSAM,
            None, // default security
            &mut hkey,
            None, // don't care if it was created vs opened
        );
        debug_assert_eq!(status, ERROR_SUCCESS, "RegCreateKeyExW failed");

        // Read each DWORD; ERROR_FILE_NOT_FOUND (or anything else that isn't
        // ERROR_SUCCESS) falls back to CW_USEDEFAULT, the "let Windows pick"
        // sentinel from e03. DWORD is unsigned, window coordinates are
        // signed — bridge with a bit-pattern-preserving cast.
        let saved_x = read_dword(hkey, w!("x"));
        let restored = saved_x.is_some();
        let x = saved_x.map(|v| v as i32).unwrap_or(CW_USEDEFAULT);
        let y = read_dword(hkey, w!("y"))
            .map(|v| v as i32)
            .unwrap_or(CW_USEDEFAULT);
        let w = read_dword(hkey, w!("w")).map(|v| v as i32).unwrap_or(640);
        let h = read_dword(hkey, w!("h")).map(|v| v as i32).unwrap_or(360);

        // ---- 2. The usual window, positioned from what we just read ------
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e93_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let title = if restored {
            "e93 \u{b7} Registry Settings \u{2014} position restored from the registry"
        } else {
            "e93 \u{b7} Registry Settings \u{2014} first run (defaults); close me and run again"
        };

        let hwnd = CreateWindowExW(
            0,
            w!("e93_class"),
            &HSTRING::from(title),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::new(State { hkey, restored }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Read one REG_DWORD value; `None` if it is missing (first run) or the
/// read fails for any other reason — caller decides the fallback.
unsafe fn read_dword(hkey: HKEY, name: PCWSTR) -> Option<u32> {
    unsafe {
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        // The API is byte-typed (LPBYTE), so we lend it a *mut u8 view of
        // our u32. lptype/lpreserved stay None — we don't need the type
        // back because we only ever write REG_DWORDs here.
        let status = RegQueryValueExW(
            hkey,
            name,
            None,
            None,
            Some(&mut data as *mut u32 as LPBYTE),
            Some(&mut size),
        );
        (status == ERROR_SUCCESS).then_some(data)
    }
}

/// Write one REG_DWORD value; errors are ignored (a settings write failing
/// should never take the app down).
unsafe fn write_dword(hkey: HKEY, name: PCWSTR, value: u32) {
    unsafe {
        _ = RegSetValueExW(
            hkey,
            name,
            None,
            REG_DWORD,
            Some(&value as *const u32 as *const u8),
            std::mem::size_of::<u32>() as u32,
        );
    }
}

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                let source = if state.restored {
                    "x, y, w, h came from HKCU\\Software\\windows-rs-tutorial\\e93"
                } else {
                    "no saved values found (ERROR_FILE_NOT_FOUND) \u{2014} CW_USEDEFAULT defaults"
                };
                let lines = [
                    "Move and resize this window, then close it.",
                    source,
                    "Run `cargo run -p e93_registry_settings` again: same place, same size.",
                    "regedit \u{2192} HKEY_CURRENT_USER\\Software\\windows-rs-tutorial\\e93 shows the four DWORDs.",
                ];
                _ = SetBkMode(hdc, TRANSPARENT);
                for (i, line) in lines.iter().enumerate() {
                    let wide = to_wide(line);
                    _ = TextOutW(
                        hdc,
                        16,
                        16 + i as i32 * 28,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                    );
                }

                _ = EndPaint(hwnd, &ps);
                0
            }

            WM_DESTROY => {
                // ---- 3. Persist the placement, then close the key ----------
                // WM_DESTROY still has a live window, so GetWindowRect works.
                // One wrinkle: a window closed while MINIMIZED reports the
                // minimized position (-32000-ish) — skip the save then so a
                // stray launch from the taskbar can't poison the settings.
                let mut rc = RECT::default();
                _ = GetWindowRect(hwnd, &mut rc);
                if !IsIconic(hwnd).as_bool() {
                    write_dword(state.hkey, w!("x"), rc.left as u32);
                    write_dword(state.hkey, w!("y"), rc.top as u32);
                    write_dword(state.hkey, w!("w"), (rc.right - rc.left) as u32);
                    write_dword(state.hkey, w!("h"), (rc.bottom - rc.top) as u32);
                }
                // The key handle is an OS resource like any HANDLE: close it
                // exactly once, on the way out.
                _ = RegCloseKey(state.hkey);
                state.hkey = HKEY::default();

                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
