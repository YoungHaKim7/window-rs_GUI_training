//! e38 — The System Menu: adding your own item to the title-bar menu
//!
//! What you learn:
//! * `GetSystemMenu(hwnd, FALSE)` — fetch the window's own menu (the one
//!   behind the icon click / Alt+Space) so you can *modify the copy*.
//!   It is not the shared Windows menu: each window gets its own, and
//!   once you touch it you own its contents
//! * `AppendMenuW` on it — a separator plus a custom item work exactly
//!   like on any popup
//! * `WM_SYSCOMMAND` — where the system menu talks. The picks arrive in
//!   wparam, but you MUST mask with `& 0xFFF0`: the low 4 bits are used
//!   internally by the system (they encode *how* the command fired for
//!   built-ins — mouse vs keyboard vs double-click). All SC_* constants
//!   are defined with the low nibble zero so the masked compare is exact
//! * Follow-up quirk: because that nibble belongs to the system, two
//!   custom ids that differ only in their low 4 bits are
//!   indistinguishable after masking — pick ids that are multiples of 16
//!   when you add more than one (our 2001 survives as 2000 after the
//!   mask, which is what we compare against)
//! * `GetSystemMenu(hwnd, TRUE)` — the escape hatch: restores the
//!   *default* system menu, wiping your additions (call FALSE again to
//!   keep customizing afterwards)
//!
//! Click the window icon (or press Alt+Space) — a custom item sits under
//! the built-ins.
//!
//! New Win32 APIs: `GetSystemMenu`, `WM_SYSCOMMAND` (winuser).
//!
//! Run: `cargo run -p e38_system_menu`

use windows::{Win32::*, core::*};

// Custom system-menu id. Ids >= 0xF000 are reserved for the system's own
// SC_* values (SC_CLOSE = 0xF060, ...); stay below.
const IDM_ABOUT_SYS: usize = 2001;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e38_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e38_class"),
            w!("e38 · System Menu — look under the window icon"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 560, 320,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // FALSE = "give me the menu so I can change it". The first call
        // duplicates the default menu for this window; after this the
        // window shows the modified copy. (TRUE instead would *reset* it
        // to the default and return the fresh handle — that's how you'd
        // undo everything below: GetSystemMenu(hwnd, TRUE) and the
        // built-ins are back, our additions gone.)
        let sys = GetSystemMenu(hwnd, false);
        debug_assert!(!sys.is_null());

        _ = AppendMenuW(sys, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(sys, MF_STRING as u32, IDM_ABOUT_SYS, w!("About the system &menu..."));

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_SYSCOMMAND => {
                // Mask BEFORE comparing: the low 4 bits are the system's
                // private flags (for SC_MOVE they distinguish "dragged
                // with the mouse" from "moved with the keyboard", etc.).
                // Our id 2001 loses its low nibble under the mask — hence
                // we compare against 2001 & 0xFFF0 = 2000, and the comment
                // above about choosing ids in multiples of 16.
                let cmd = (wparam & 0xfff0) as usize;

                if cmd == IDM_ABOUT_SYS & 0xfff0 {
                    _ = MessageBoxW(
                        Some(hwnd),
                        w!("You picked the custom item in the system menu.\n\nIt arrived as WM_SYSCOMMAND with wparam masked by 0xFFF0."),
                        w!("System menu item"),
                        (MB_OK | MB_ICONINFORMATION) as u32,
                    );
                    0 // handled — do NOT fall through for our own item
                } else {
                    // Everything else (Close, Move, Size, Minimize...) must
                    // reach DefWindowProcW — that's where the built-in
                    // behavior lives.
                    DefWindowProcW(hwnd, message, wparam, lparam)
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
                    w!("Click the icon at the top-left (or press Alt+Space):\na separator and \"About the system menu\" sit under the built-ins."),
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
