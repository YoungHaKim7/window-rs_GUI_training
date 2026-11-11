//! e36 — Dynamic Menus: growing and pruning a submenu at runtime
//!
//! What you learn:
//! * Menus are *live objects*: `AppendMenuW` can add items any time, not
//!   just at startup — here a "Recent" submenu grows as you work
//! * `RemoveMenu` — delete an item, by **id** (`MF_BYCOMMAND`) or by
//!   **zero-based index** (`MF_BYPOSITION`). The distinction matters when
//!   ids churn: our recents get fresh ids every run, so deleting "the
//!   oldest" is trivially `RemoveMenu(menu, 0, MF_BYPOSITION)` — position
//!   0 — whereas "find the id of the oldest first" is bookkeeping you
//!   don't need
//! * A capacity policy enforced in code: the list is capped at 5; adding
//!   a 6th evicts the oldest first (FIFO — the classic recents-files UX)
//! * `GetMenuItemCount` — read back how many items a menu currently has
//!   (the menu is state too; don't be shy about querying it)
//!
//! Pick File → Add recent item a few times, then open the Recent submenu
//! and watch it grow, cap at five, and start sliding.
//!
//! New Win32 APIs: `RemoveMenu`, `GetMenuItemCount`, `DrawMenuBar`
//! (winuser).
//!
//! Run: `cargo run -p e36_dynamic_menu`

use windows::{Win32::*, core::*};

const ID_ADD_RECENT: usize = 4001;
const ID_FIRST_RECENT: usize = 4100; // generated ids start here
const CAP: usize = 5; // recents capacity

#[derive(Default)]
struct State {
    counter: u32, // how many recents were ever added (for names + ids)
    /// (id, label) of the items currently in the submenu.
    recents: Vec<(usize, String)>,
    picked: Option<String>, // the recent the user last picked
    recent_menu: HMENU,
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
            lpszClassName: w!("e36_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e36_class"),
            w!("e36 · Dynamic Menu — a Recent list that grows"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 620, 400,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // ---- Menu: File owns the action item and the Recent submenu ----
        let bar = CreateMenu();
        let file_menu = CreatePopupMenu();
        _ = AppendMenuW(file_menu, MF_STRING as u32, ID_ADD_RECENT, w!("&Add recent item"));
        _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));
        let recent_menu = CreatePopupMenu(); // starts empty — it fills at runtime
        _ = AppendMenuW(file_menu, MF_POPUP as u32, recent_menu as usize, w!("&Recent"));
        _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
        _ = SetMenu(hwnd, Some(bar));

        let state = Box::into_raw(Box::new(State {
            recent_menu,
            ..Default::default()
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        _ = DestroyMenu(bar);
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let id = (wparam & 0xffff) as usize;

                if id == ID_ADD_RECENT {
                    // Evict before append: position 0 is the OLDEST entry
                    // (items stack downward as they're appended).
                    // MF_BYPOSITION says "the first argument is an index,
                    // not an id" — with MF_BYCOMMAND it would be an id.
                    if state.recents.len() == CAP {
                        _ = RemoveMenu(state.recent_menu, 0, MF_BYPOSITION as u32);
                        state.recents.remove(0);
                    }

                    state.counter += 1;
                    let id = ID_FIRST_RECENT + state.counter as usize; // fresh unique id
                    let label = format!("File {}.txt", state.counter);

                    // AppendMenuW takes the label as a wide string; HSTRING
                    // converts from our runtime-built name.
                    _ = AppendMenuW(
                        state.recent_menu,
                        MF_STRING as u32,
                        id,
                        &HSTRING::from(&label),
                    );
                    state.recents.push((id, label));

                    // Popups refresh themselves when next opened; the bar
                    // only changes if we add/remove top-level entries.
                    // DrawMenuBar is the belt-and-braces refresh for the
                    // rare case Windows hasn't noticed a bar change.
                    _ = DrawMenuBar(hwnd);

                    _ = InvalidateRect(Some(hwnd), None, true);
                    0
                } else if id > ID_FIRST_RECENT && id <= ID_FIRST_RECENT + state.counter as usize {
                    // One of the generated recents: find its label.
                    if let Some((_, label)) = state.recents.iter().find(|(rid, _)| *rid == id) {
                        state.picked = Some(label.clone());
                        _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                _ = SetBkMode(hdc, TRANSPARENT);

                // Current capacity readout — straight from the menu.
                let count = GetMenuItemCount(Some(state.recent_menu));
                let head = format!("Recent submenu holds {count} of {CAP} items");
                let wide = to_wide(&head);
                _ = SetTextColor(hdc, rgb(0x20, 0x40, 0x80));
                _ = TextOutW(hdc, 20, 20, PCWSTR::from_raw(wide.as_ptr()), -1);

                // The list itself, oldest at top (matching menu order).
                _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));
                for (i, (_, label)) in state.recents.iter().enumerate() {
                    let wide = to_wide(label);
                    _ = TextOutW(hdc, 20, 52 + i as i32 * 24, PCWSTR::from_raw(wide.as_ptr()), -1);
                }

                // The pick, if any, in a louder color at the bottom.
                if let Some(picked) = &state.picked {
                    let line = format!("opened: {picked}");
                    let wide = to_wide(&line);
                    _ = SetTextColor(hdc, rgb(0xB0, 0x30, 0x30));
                    _ = TextOutW(hdc, 20, rc.bottom - 40, PCWSTR::from_raw(wide.as_ptr()), -1);
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
