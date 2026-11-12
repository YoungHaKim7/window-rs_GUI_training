//! e37 — Keyboard Accelerators: Ctrl+O and Ctrl+X, the right way
//!
//! What you learn:
//! * `CreateAcceleratorTableW` + the `ACCEL` array — a table of
//!   "modifier + key → command id" rules built entirely in code (the
//!   classic alternative is an accelerator resource in the .rc file)
//! * `TranslateAcceleratorW` FIRST in the message pump, before
//!   `TranslateMessage`/`DispatchMessageW` — and *why* the order is not
//!   negotiable: it must inspect raw `WM_KEYDOWN` messages and swallow
//!   the ones it recognizes, re-posting them as `WM_COMMAND`. One line
//!   later and DispatchMessageW would already have delivered the key
//!   to your wndproc as an ordinary keystroke
//! * The elegant trick: give the accelerator the SAME id as a menu item
//!   — now menu pick and keystroke funnel into one WM_COMMAND handler,
//!   and the menu even displays "Ctrl+O" next to the item for free
//!   (the `\t` in the label aligns the hint into a right column)
//!
//! Press Ctrl+O (about box) and Ctrl+X (exit) — or use the menu, the
//! result is identical.
//!
//! New Win32 APIs: `CreateAcceleratorTableW`, `TranslateAcceleratorW`,
//! `DestroyAcceleratorTable`, the `ACCEL` struct (winuser).
//!
//! Run: `cargo run -p e37_accelerators`

use windows::{Win32::*, core::*};

// Ids shared by the menu items AND the accelerator entries — the whole
// point: one WM_COMMAND handler, two input surfaces.
const ID_OPEN: u16 = 5001;
const ID_EXIT: u16 = 5002;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ---- The accelerator table --------------------------------------
        // fVirt is a bit field of F* flags: FVIRTKEY = `key` is a virtual
        // key code (not a character), FCONTROL/FSHIFT/FALT = modifiers.
        let mut accels = [
            ACCEL {
                fVirt: (FVIRTKEY | FCONTROL) as u8, // Ctrl, virtual key
                key: 'O' as u16,                    // virtual-key code for O
                cmd: ID_OPEN,
            },
            ACCEL {
                fVirt: (FVIRTKEY | FCONTROL) as u8,
                key: 'X' as u16,
                cmd: ID_EXIT,
            },
        ];
        let haccel = CreateAcceleratorTableW(accels.as_mut_ptr(), accels.len() as i32);
        debug_assert!(!haccel.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e37_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e37_class"),
            w!("e37 · Accelerators — Ctrl+O / Ctrl+X"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            320,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // Menu items carrying the SAME ids — the "\t" puts the shortcut
        // hint into the menu's right-hand column automatically.
        let bar = CreateMenu();
        let file_menu = CreatePopupMenu();
        _ = AppendMenuW(
            file_menu,
            MF_STRING as u32,
            ID_OPEN as usize,
            w!("Open...\tCtrl+O"),
        );
        _ = AppendMenuW(
            file_menu,
            MF_STRING as u32,
            ID_EXIT as usize,
            w!("Exit\tCtrl+X"),
        );
        _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
        _ = SetMenu(hwnd, Some(bar));

        // ---- The pump, in the one correct order ------------------------
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            // 1) Accelerators first. If the message is a key press that
            //    matches the table, this call POSTS a WM_COMMAND with the
            //    accelerator's id to our hwnd and returns nonzero — we
            //    `continue` so the swallowed key never reaches
            //    TranslateMessage (which would turn it into WM_CHAR) or
            //    wndproc (which would see WM_KEYDOWN).
            if TranslateAcceleratorW(hwnd, haccel, &mut message) != 0 {
                continue;
            }
            // 2) Only then the usual pair.
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // The table is a handle like any other; free it on the way out.
        _ = DestroyAcceleratorTable(haccel);
        _ = DestroyMenu(bar);
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_COMMAND => {
                // Both roads end here: menu pick and Ctrl+key. For a
                // menu-source WM_COMMAND lparam is 0; for an accelerator
                // it is 1 (see docs for WM_COMMAND) — the id is all we
                // ever need.
                match (wparam & 0xffff) as u16 {
                    ID_OPEN => {
                        _ = MessageBoxW(
                            Some(hwnd),
                            w!(
                                "Ctrl+O or the menu — either way you got here through one WM_COMMAND with id 5001."
                            ),
                            w!("Open (accelerator)"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        0
                    }
                    ID_EXIT => {
                        _ = DestroyWindow(hwnd);
                        0
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                let mut hint = rc;
                DrawTextW(
                    hdc,
                    w!(
                        "Press Ctrl+O for the box, Ctrl+X to exit.\n\nThe menu items below the title bar share the same ids — one handler, two triggers."
                    ),
                    -1,
                    &mut hint,
                    (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
                );
                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
