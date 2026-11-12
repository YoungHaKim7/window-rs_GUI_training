//! e33 — Menu Bar: CreateMenu, AppendMenuW, SetMenu, WM_COMMAND
//!
//! What you learn:
//! * The menu-bar recipe: `CreateMenu` (an empty bar) + one
//!   *per menu* `CreatePopupMenu` (a drop-down), `AppendMenuW` the drop-downs
//!   into the bar, then `SetMenu(hwnd, bar)` to attach the whole thing
//! * `WM_COMMAND` — how a menu pick arrives: LOWORD(wparam) is the item's
//!   id, HIWORD(wparam) is 0 when the source is a menu (controls put their
//!   notification code there — the same message serves both)
//! * Menu ids are just numbers you pick (≥ 0xF000 is reserved for the
//!   system's own commands — steer clear)
//! * `&Exit` strings: the `&` marks the Alt-access key and underlines it
//!
//! Open File → Exit to close the window, Help → About for a MessageBox.
//!
//! New Win32 APIs: `CreateMenu`, `CreatePopupMenu`, `AppendMenuW`,
//! `SetMenu`, `DestroyMenu` (winuser).
//!
//! Run: `cargo run -p e33_menu_bar`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e33_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e33_class"),
            w!("e33 · Menu Bar — File / Help"),
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

        // ---- Build the menu bar --------------------------------------
        // The bar itself: a horizontal container that owns nothing.
        let bar = CreateMenu();

        // File drop-down. CreatePopupMenu makes an initially empty
        // *popup* — the term covers both drop-downs and context menus.
        let file_menu = CreatePopupMenu();
        _ = AppendMenuW(
            file_menu,
            MF_STRING as u32, // a plain text item
            1001,             // the id we will read back in WM_COMMAND
            w!("E&xit"),      // &x = Alt,F then X works from the keyboard
        );

        let help_menu = CreatePopupMenu();
        _ = AppendMenuW(help_menu, MF_STRING as u32, 1002, w!("&About..."));

        // Append the drop-downs to the bar. Note the role swap versus the
        // items above: for a submenu, id slot carries the CHILD HMENU and
        // the string slot carries the label — that is what MF_POPUP means.
        _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
        _ = AppendMenuW(bar, MF_POPUP as u32, help_menu as usize, w!("&Help"));

        // Attach: the bar appears under the title bar immediately (Windows
        // also takes ownership of drawing, keyboard navigation, Alt
        // highlighting — we only ever see the resulting WM_COMMAND).
        _ = SetMenu(hwnd, Some(bar));

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // Menus are GDI-ish objects owned by us once detached; the doc-
        // pattern is DestroyMenu on the bar (it recursively frees the
        // submenus attached to it).
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
                // Every menu pick lands here. The two halves of wparam:
                //   LOWORD = the item id we chose in AppendMenuW
                //   HIWORD = 0 for menus, the notification code for
                //            controls (buttons send BN_CLICKED = 0 too)
                // lparam = the control HWND — 0 for menu picks.
                let id = wparam & 0xffff;
                match id {
                    1001 => {
                        // Exit: route through the same door as the X
                        // button so cleanup stays in one place.
                        _ = DestroyWindow(hwnd);
                        0
                    }
                    1002 => {
                        _ = MessageBoxW(
                            Some(hwnd),
                            w!(
                                "A menu bar, built with AppendMenuW and attached with SetMenu.\n\nThe pick arrived as WM_COMMAND with id 1002 in LOWORD(wparam)."
                            ),
                            w!("About e33"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
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
                        "Try the bar above: File → Exit, Help → About.\n\nShortcuts work too: Alt+F opens File, then X exits."
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
