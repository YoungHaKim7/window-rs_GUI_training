//! e67 — Custom Control: your own reusable child window class
//!
//! What you learn:
//! * How to package behavior into a **reusable control**: register a class
//!   with its own wndproc, create instances as `WS_CHILD` windows, done.
//!   This is *exactly* how BUTTON and EDIT are built — they hold no magic
//!   you don't have. Your "ColorTile" is a peer of "BUTTON".
//! * The control-notification contract: a control reports to its parent
//!   with `SendMessageW(parent, WM_COMMAND, ...)`, packing its control id
//!   (LOWORD of wparam), a notification code (HIWORD), and its own hwnd
//!   (lparam). The parent can then `GetDlgItem`-style look the child up.
//!   That shape is what every `BN_CLICKED` / `EN_CHANGE` you've handled
//!   already was doing behind the curtain.
//! * `WM_PARENTNOTIFY` exists too (parents get it for clicks etc. on
//!   children), but real controls talk WM_COMMAND — we follow the real
//!   convention so our tiles plug into ordinary WM_COMMAND handling.
//! * A custom notification code: pick an id above `WM_APP`-style ranges —
//!   here `TILE_CLICKED` — so it can't collide with 0 (BN_CLICKED) etc.
//!
//! Click tiles to cycle their colors; the status line under them shows the
//! WM_COMMAND traffic from the last click, decoded like a parent decodes
//! any control message.
//!
//! New Win32 APIs: `WM_COMMAND` from the *sender's* side,
//! `MAKEWPARAM`-style packing (done by hand), `GetWindowTextW` reuse.
//!
//! Run: `cargo run -p e67_custom_control`

use windows::{Win32::*, core::*};

/// The control's class name — what a consumer passes to CreateWindowExW,
/// exactly like `w!("BUTTON")`. Prefix with something app-specific so it
/// can't collide with system classes.
const TILE_CLASS: PCWSTR = w!("e67_ColorTile");

/// The control's own notification code. System controls use small numbers
/// (BN_CLICKED = 0, EN_CHANGE = 0x300); to stay collision-free a custom
/// control should pick something in the WM_APP-ish private space.
const TILE_CLICKED: i32 = 0x8000;

/// Palette the tile cycles through. COLORREF layout is 0x00BBGGRR.
const TILE_COLORS: [COLORREF; 4] = [
    0x0000_00C0, // red
    0x00C0_0000, // blue
    0x00C0_C000, // green
    0x0000_80FF, // orange
];

const TILE_IDS: [i32; 4] = [101, 102, 103, 104];

struct MainState {
    last_report: String,
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ---- The control's class -------------------------------------
        // Registered once, instanced many times. Note there is nothing
        // special here — a class is a class, whether it makes top-level
        // windows or controls.
        let tile_class = WNDCLASSW {
            lpfnWndProc: Some(tile_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_HAND.0.cast())),
            hbrBackground: std::ptr::null_mut(), // we paint everything
            lpszClassName: TILE_CLASS,
            style: (CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&tile_class) != 0);

        // ---- The consumer's class (an ordinary main window) ----------
        let main_class = WNDCLASSW {
            lpfnWndProc: Some(main_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e67_main"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&main_class) != 0);

        let state = Box::into_raw(Box::new(MainState {
            last_report: "click a tile".to_string(),
        }));
        let hwnd = CreateWindowExW(
            0,
            w!("e67_main"),
            w!("e67 · Custom Control — ColorTile"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            240,
            None,
            None,
            Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!hwnd.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe fn main_state_of(hwnd: HWND) -> Option<&'static mut MainState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainState;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

/// Lay out the four tiles in a row, called from WM_CREATE and WM_SIZE —
/// layout ownership stays with the *parent*, which knows its client size
/// (the splitter example, e70, formalizes that idea).
unsafe fn layout_tiles(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        _ = GetClientRect(hwnd, &mut rect);
        let gap = 16;
        let top = 20;
        let bottom = rect.bottom - 70;
        let height = bottom - top;
        let width = (rect.right - 4 * gap) / 4;

        for (i, id) in TILE_IDS.iter().enumerate() {
            let tile = GetDlgItem(Some(hwnd), *id);
            if !tile.is_null() {
                _ = MoveWindow(
                    tile,
                    gap + i as i32 * (width + gap),
                    top,
                    width,
                    height,
                    true,
                );
            }
        }
    }
}

extern "system" fn main_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_CREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;

                for (i, id) in TILE_IDS.iter().enumerate() {
                    let tile = CreateWindowExW(
                        0,
                        TILE_CLASS, // the custom class, like "BUTTON"
                        w!(""),     // window text: unused by ColorTile
                        (WS_CHILD | WS_VISIBLE) as u32,
                        0,
                        0,
                        0,
                        0, // positioned by layout_tiles
                        Some(hwnd),
                        Some(*id as usize as HMENU), // control id (fake-HMENU trick)
                        Some(instance),
                        None,
                    );
                    debug_assert!(!tile.is_null());
                    // Seed the tile's color from its control id — a cheap
                    // per-instance parameter with no lpparam plumbing.
                    SetWindowLongPtrW(tile, GWLP_USERDATA, (i % TILE_COLORS.len()) as isize);
                }
                layout_tiles(hwnd);
                0
            }
            WM_SIZE => {
                layout_tiles(hwnd);
                0
            }
            WM_COMMAND => {
                // Decode like any other control notification:
                //   LOWORD(wparam) = control id
                //   HIWORD(wparam) = notification code
                //   lparam         = the control's own hwnd
                let id = (wparam & 0xffff) as i32;
                let code = ((wparam >> 16) & 0xffff) as i32;
                let from = lparam as HWND;
                if code == TILE_CLICKED && TILE_IDS.contains(&id) {
                    let report = if let Some(state) = main_state_of(hwnd) {
                        // Reading the child's GWLP_USERDATA directly is
                        // fine within one process — it's our contract.
                        let color = GetWindowLongPtrW(from, GWLP_USERDATA) as usize;
                        state.last_report = format!(
                            "WM_COMMAND: id={} code=TILE_CLICKED tile=#{:?} color={}",
                            id,
                            TILE_IDS
                                .iter()
                                .position(|&x| x == id)
                                .map(|n| n + 1)
                                .unwrap_or(0),
                            color
                        );
                        state.last_report.clone()
                    } else {
                        return 0;
                    };
                    _ = InvalidateRect(Some(hwnd), None, true);
                    let _ = report;
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                if let Some(state) = main_state_of(hwnd) {
                    let mut r = rect;
                    r.top = rect.bottom - 50;
                    _ = SetBkMode(hdc, TRANSPARENT);
                    let wide = to_wide(&state.last_report);
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut r,
                        (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                    );
                }

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainState;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

// ======================================================================
// The ColorTile control itself
// ======================================================================
extern "system" fn tile_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            // The whole per-instance state is "which palette index am I
            // on" — a plain number, so GWLP_USERDATA stores it directly,
            // no Box needed. (The parent seeds it after creation.)
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                let index = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize % TILE_COLORS.len();
                let brush = CreateSolidBrush(TILE_COLORS[index]);
                _ = FillRect(hdc, &rect, brush);
                _ = DeleteObject(brush as HGDIOBJ);

                // A thin frame so adjacent tiles read as separate.
                let frame = GetSysColorBrush(COLOR_WINDOWFRAME);
                _ = FrameRect(hdc, &rect, frame);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_LBUTTONDOWN => {
                // Cycle, repaint, and REPORT — the heart of being a
                // control. A control never assumes who listens; it just
                // speaks WM_COMMAND and lets any parent decide.
                let next =
                    (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize + 1) % TILE_COLORS.len();
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, next as isize);
                _ = InvalidateRect(Some(hwnd), None, true);

                let parent = GetParent(hwnd);
                // Packing, by hand:
                //   wparam = MAKEWPARAM(control_id, notification)
                //   lparam = control hwnd
                let id = GetWindowLongPtrW(hwnd, GWLP_ID) as usize & 0xffff;
                let packed = id | ((TILE_CLICKED as usize & 0xffff) << 16);
                _ = SendMessageW(parent, WM_COMMAND as u32, packed, hwnd as isize);

                // WM_PARENTNOTIFY is the other direction parents can
                // listen on; we deliberately use WM_COMMAND because it is
                // what BUTTON/EDIT do and what parents already route.
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
