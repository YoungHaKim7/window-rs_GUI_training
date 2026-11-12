//! e71 — Tray Icon: Shell_NotifyIconW and the notification area
//!
//! What you learn:
//! * `Shell_NotifyIconW` — the notification-area ("system tray") API. One
//!   `NOTIFYICONDATAW` struct describes the icon; the first argument
//!   selects the verb: `NIM_ADD` installs it, `NIM_MODIFY` changes it
//!   (icon, tooltip, or a balloon), `NIM_DELETE` removes it.
//! * The callback contract: with `uFlags = NIF_MESSAGE` and
//!   `uCallbackMessage = WM_APP + 1`, mouse activity on the icon arrives
//!   as that message at your window — **lparam carries the mouse message**
//!   (WM_LBUTTONDOWN, WM_RBUTTONUP, WM_MOUSEMOVE...), wparam the icon id.
//!   WM_APP..WM_APP+0xBFFF is the documented app-private message range.
//! * The two famous quirks, both about focus:
//!     - restoring from the tray needs `SetForegroundWindow` BEFORE
//!       `ShowWindow`-driven activation, or the window comes up behind
//!       whatever was foreground;
//!     - a `TrackPopupMenu` shown from a tray click needs the owner
//!       foregrounded FIRST and a `WM_NULL` posted AFTER, or the menu
//!       refuses to close when you click elsewhere (KB135788 classic).
//! * Balloons: `NIM_MODIFY` + `NIF_INFO` + `szInfo`/`szInfoTitle` — the
//!   toast of the WinXP era, still the zero-dependency option.
//! * Why `NIM_DELETE` in WM_DESTROY matters: skip it and a *ghost icon*
//!   stays in the tray until the next hover (Windows only notices the
//!   window is gone when it tries to deliver the callback message).
//!
//! Minimize the window (it hides to the tray), then: left-click the tray
//! icon to restore, right-click it for the menu (Balloon / Restore / Exit).
//!
//! New Win32 APIs: `Shell_NotifyIconW`, `NOTIFYICONDATAW`,
//! `NIM_ADD`/`NIM_MODIFY`/`NIM_DELETE`, `NIF_*`, `TrackPopupMenu`.
//!
//! Run: `cargo run -p e71_tray_icon`

use windows::{Win32::*, core::*};

/// Our private callback message (WM_APP range = reserved for the app).
const WM_APP_TRAY: i32 = WM_APP + 1;

/// The icon id: only needed if you ever add several icons at once.
const TRAY_ID: u32 = 1;

/// Menu item ids for the tray context menu.
const IDM_BALLOON: usize = 2001;
const IDM_RESTORE: usize = 2002;
const IDM_EXIT: usize = 2003;

#[derive(Default)]
struct State {
    /// Trick balloon counter, so repeated balloons have varying text.
    balloons: u32,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e71_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e71_class"),
            w!("e71 · Tray Icon — minimize me, then use the tray"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            300,
            None,
            None,
            Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!hwnd.is_null());
        if hwnd.is_null() {
            drop(Box::from_raw(state));
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
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

/// Copy a Rust string into a fixed [u16; N] tooltip/info field, truncating
/// if needed (the arrays are short — 128/64/256 chars — and NOT copying a
/// terminator would read garbage).
fn copy_into_fixed(target: &mut [u16], text: &str) {
    let mut i = 0;
    for u in text.encode_utf16() {
        if i + 1 >= target.len() {
            break; // leave room for the terminator
        }
        target[i] = u;
        i += 1;
    }
    target[i] = 0;
}

/// One call, all verbs: fill the struct, pick NIM_*.
unsafe fn shell_notify(
    hwnd: HWND,
    verb: u32,
    flags: u32,
    tip: Option<&str>,
    info: Option<(&str, &str)>,
) {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd, // who receives the callback message
            uID: TRAY_ID, // our id among this window's icons
            uFlags: flags, // which members of this struct are meaningful
            uCallbackMessage: WM_APP_TRAY as u32, // mouse events arrive as this
            hIcon: LoadIconW(None, PCWSTR(IDI_APPLICATION.0.cast())),
            ..Default::default()
        };

        if let Some(tip) = tip {
            copy_into_fixed(&mut nid.szTip, tip);
        }
        if let Some((title, body)) = info {
            copy_into_fixed(&mut nid.szInfoTitle, title);
            copy_into_fixed(&mut nid.szInfo, body);
            // dwInfoFlags = NIIF_INFO would pick the info glyph; the
            // default (no flag) already shows the standard balloon icon.
            nid.dwInfoFlags = 0;
        }

        _ = Shell_NotifyIconW(verb, &mut nid);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_CREATE => {
                // Install the tray icon: message-callback + icon + tooltip.
                shell_notify(
                    hwnd,
                    NIM_ADD as u32,
                    (NIF_MESSAGE | NIF_ICON | NIF_TIP) as u32,
                    Some("e71 tray icon (right-click me)"),
                    None,
                );
                0
            }

            // Minimizing hides the window instead — the tray icon is now
            // the only way back. (Real apps do this behind a setting.)
            WM_SIZE => {
                if wparam as i32 == SIZE_MINIMIZED {
                    _ = ShowWindow(hwnd, SW_HIDE);
                }
                0
            }

            // ---- Mouse activity on the tray icon -------------------------
            // Contract: wparam = icon id, lparam = the mouse message that
            // happened over the icon (as the LOWORD).
            WM_APP_TRAY => {
                let mouse_msg = (lparam & 0xffff) as i32;
                match mouse_msg {
                    // Left click → restore. The quirk: a tray click does
                    // NOT make you the foreground window, so the restored
                    // window would surface *behind* the current one. Call
                    // SetForegroundWindow yourself, first.
                    WM_LBUTTONUP => {
                        _ = SetForegroundWindow(hwnd);
                        _ = ShowWindow(hwnd, SW_RESTORE);
                        0
                    }

                    // Right click → context menu at the cursor. Same focus
                    // story, plus its sibling quirk: unless the menu OWNER
                    // is foreground, the menu ignores outside clicks and
                    // stays open forever. Foreground first, and post the
                    // WM_NULL after — the KB-recommended workaround.
                    WM_RBUTTONUP => {
                        _ = SetForegroundWindow(hwnd);

                        let menu = CreatePopupMenu();
                        _ = AppendMenuW(menu, MF_STRING as u32, IDM_BALLOON, w!("Show a &balloon"));
                        _ = AppendMenuW(menu, MF_STRING as u32, IDM_RESTORE, w!("&Restore"));
                        _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, w!(""));
                        _ = AppendMenuW(menu, MF_STRING as u32, IDM_EXIT, w!("E&xit"));

                        let mut pt = POINT::default();
                        _ = GetCursorPos(&mut pt);
                        // TPM_RETURNCMD: the chosen id comes back from the
                        // call itself (instead of as WM_COMMAND) — handy in
                        // a tray menu with no menu bar in sight.
                        // TPM_BOTTOMALIGN: pop the menu UP from the cursor,
                        // the tray convention.
                        let picked = TrackPopupMenu(
                            menu,
                            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN) as u32,
                            pt.x,
                            pt.y,
                            None,
                            hwnd,
                            None,
                        )
                        .0;

                        // Quirk part 2: the WM_NULL lets the menu finish
                        // its bookkeeping so the outside-click dismissal
                        // works even though focus instantly moves on.
                        _ = PostMessageW(Some(hwnd), WM_NULL as u32, 0, 0);

                        match picked as usize {
                            IDM_BALLOON => {
                                let n = if let Some(state) = state_of(hwnd) {
                                    state.balloons += 1;
                                    state.balloons
                                } else {
                                    0
                                };
                                // Balloon = NIM_MODIFY with NIF_INFO and the
                                // two text fields filled. It replaces the
                                // icon's passive tooltip for its lifetime.
                                shell_notify(
                                    hwnd,
                                    NIM_MODIFY as u32,
                                    NIF_INFO as u32,
                                    None,
                                    Some((
                                        "e71 says hi",
                                        &format!("Balloon #{} — NIM_MODIFY + NIF_INFO.", n),
                                    )),
                                );
                            }
                            IDM_RESTORE => {
                                _ = SetForegroundWindow(hwnd);
                                _ = ShowWindow(hwnd, SW_RESTORE);
                            }
                            IDM_EXIT => {
                                _ = DestroyWindow(hwnd);
                            }
                            _ => {}
                        }

                        _ = DestroyMenu(menu);
                        0
                    }
                    _ => 0, // moves, double-clicks... not used here
                }
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                _ = SetBkMode(hdc, TRANSPARENT);
                let wide = to_wide(
                    "1. Minimize this window — it hides instead.\n\
                     2. Left-click the tray icon: restore (note SetForegroundWindow).\n\
                     3. Right-click it: menu with a balloon, restore, exit.",
                );
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut rc,
                    (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
                );
                _ = EndPaint(hwnd, &ps);
                0
            }

            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_DESTROY => {
                // Remove the icon BEFORE the window dies. If you skip this,
                // a ghost icon lingers in the tray: Windows only reaps it
                // when the next hover exposes the dead callback target.
                shell_notify(hwnd, NIM_DELETE as u32, 0, None, None);
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
