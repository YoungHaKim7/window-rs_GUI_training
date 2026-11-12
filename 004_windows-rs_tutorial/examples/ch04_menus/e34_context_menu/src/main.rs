//! e34 — Context Menu: right-click, TrackPopupMenu, TPM_RETURNCMD
//!
//! What you learn:
//! * `WM_CONTEXTMENU` — the modern way to learn "the user asked for a
//!   menu here": fires on right-click, Shift+F10, and the menu key.
//!   wparam is the clicked window, lparam packs the *screen* coordinates
//!   (signed 16-bit halves; (-1,-1) means "keyboard invoked — no cursor")
//! * `TrackPopupMenu` — display a popup at the cursor and run its own
//!   mini message loop until the user picks or dismisses
//! * `TPM_RETURNCMD` — the star of this example: TrackPopupMenu *returns*
//!   the chosen id instead of posting `WM_COMMAND` to your wndproc. You
//!   get synchronous, local handling — no routing through the message
//!   pump. (Without it, the pick arrives as WM_COMMAND with the id in
//!   LOWORD(wparam), exactly like a menu bar — that's the alternative
//!   style, and the one you must use for popups owned by dialogs)
//! * `CreatePopupMenu` builds a fresh popup each time; `DestroyMenu`
//!   frees it after (popups are yours to own, not cached by the OS)
//!
//! Right-click anywhere in the window to choose a color.
//!
//! New Win32 APIs: `WM_CONTEXTMENU`, `CreatePopupMenu`,
//! `TrackPopupMenu(TPM_RETURNCMD)`, `DestroyMenu` (winuser).
//!
//! Run: `cargo run -p e34_context_menu`

use windows::{Win32::*, core::*};

/// Menu ids for the popup items (any values you like; documented here).
const ID_CRIMSON: usize = 2001;
const ID_SEAGREEN: usize = 2002;
const ID_ORANGE: usize = 2003;
const ID_CLEAR: usize = 2004;

#[derive(Default)]
struct State {
    /// The color swatch currently shown in the client area.
    color: Option<(COLORREF, &'static str)>,
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
            lpszClassName: w!("e34_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e34_class"),
            w!("e34 · Context Menu — right-click me"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            360,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Build the popup, show it, and act on the synchronous result.
unsafe fn show_context_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu();
        _ = AppendMenuW(menu, MF_STRING as u32, ID_CRIMSON, w!("Crimson"));
        _ = AppendMenuW(menu, MF_STRING as u32, ID_SEAGREEN, w!("Sea green"));
        _ = AppendMenuW(menu, MF_STRING as u32, ID_ORANGE, w!("Dark orange"));
        _ = AppendMenuW(menu, MF_SEPARATOR as u32, 0, w!("")); // visual rule
        _ = AppendMenuW(menu, MF_STRING as u32, ID_CLEAR, w!("Clear"));

        // TPM_RETURNCMD flips the return value from "did it show?" (BOOL)
        // to "which item did the user pick?" (its id; 0 = dismissed).
        // TPM_LEFTALIGN pins the menu's left edge to the cursor,
        // TPM_RIGHTBUTTON lets either button pick an item.
        let picked = TrackPopupMenu(
            menu,
            (TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON) as u32,
            x,
            y,
            None, // reserved: "must be zero" says the docs, plainly None
            hwnd,
            None, // prevent-click-outside rect; None = whole screen
        )
        .0 as usize;

        // We own this popup — free it as soon as it's dismissed.
        _ = DestroyMenu(menu);

        // Because of TPM_RETURNCMD the response is handled RIGHT HERE —
        // compare with e33 where the same choice arrived as WM_COMMAND.
        if let Some(state) = state_of(hwnd) {
            state.color = match picked {
                ID_CRIMSON => Some((rgb(0xDC, 0x14, 0x3C), "Crimson")),
                ID_SEAGREEN => Some((rgb(0x2E, 0x8B, 0x57), "Sea green")),
                ID_ORANGE => Some((rgb(0xFF, 0x8C, 0x00), "Dark orange")),
                ID_CLEAR => None,
                _ => return, // 0 = dismissed without a pick — keep state
            };
            _ = InvalidateRect(Some(hwnd), None, true);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_CONTEXTMENU => {
                // lparam holds SCREEN coordinates (the menu may span
                // monitors, so client coords would be wrong). Both halves
                // are signed: multi-monitor setups place screens at
                // negative positions.
                let x = (lparam & 0xffff) as i16 as i32;
                let y = ((lparam >> 16) & 0xffff) as i16 as i32;
                if x == -1 && y == -1 {
                    // Keyboard invocation (Shift+F10 / menu key): no
                    // cursor — fall back to the client's top-left, nudged
                    // in a little so the menu isn't flush with the edge.
                    let mut rc = RECT::default();
                    _ = GetClientRect(hwnd, &mut rc);
                    let mut pt = POINT {
                        x: rc.left + 24,
                        y: rc.top + 24,
                    };
                    _ = ClientToScreen(hwnd, &mut pt);
                    show_context_menu(hwnd, pt.x, pt.y);
                } else {
                    show_context_menu(hwnd, x, y);
                }
                0
            }
            // Grandfathered alternative: handle WM_RBUTTONUP yourself and
            // convert the client point with ClientToScreen. WM_CONTEXTMENU
            // (above) is preferred — it covers right-click AND keyboard in
            // one place. If you do use WM_RBUTTONUP, remember DefWindowProc
            // turns it into WM_CONTEXTMENU anyway, so you'd see the menu
            // TWICE unless you return 0 there.
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                if let Some((color, name)) = state.color {
                    // Fill the client area with the chosen color, then
                    // caption it — the "state → paint" flow you know.
                    let brush = CreateSolidBrush(color);
                    _ = FillRect(hdc, &rc, brush);
                    _ = DeleteObject(brush as HGDIOBJ);
                    let wide = to_wide(name);
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF));
                    _ = TextOutW(hdc, 12, 12, PCWSTR::from_raw(wide.as_ptr()), -1);
                } else {
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                    let mut hint = rc;
                    DrawTextW(
                        hdc,
                        w!(
                            "Right-click anywhere (or press Shift+F10) for a context menu.\n\nTPM_RETURNCMD makes TrackPopupMenu return the pick instead of posting WM_COMMAND."
                        ),
                        -1,
                        &mut hint,
                        (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
                    );
                }

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
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
