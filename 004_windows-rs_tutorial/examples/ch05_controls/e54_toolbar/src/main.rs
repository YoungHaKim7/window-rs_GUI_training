//! e54 — Toolbar: standard system images, TBBUTTON, buttons that click
//!
//! What you learn:
//! * The toolbar (`ToolbarWindow32`, exported as `TOOLBARCLASSNAMEW`) and
//!   its four-step birth ritual: create, **`TB_BUTTONSTRUCTSIZE`** (tell
//!   comctl32 which TBBUTTON vintage you compiled against), **`TB_LOADIMAGES`**
//!   (borrow the standard 16-color icon strip — no .rc, no icons of your
//!   own), `TB_ADDBUTTONSW` (describe buttons with TBBUTTON structs)
//! * The `TBBUTTON` struct: `iBitmap` = image index (here the STD_*
//!   constants name the well-known slots), `idCommand` = the id that comes
//!   back in `WM_COMMAND`, `fsState`/`fsStyle` = enabled etc. / plain button
//! * Toolbar clicks arrive as ordinary `WM_COMMAND`s — same channel as menu
//!   items and buttons, which is THE point: a toolbar button and its menu
//!   item share one id
//! * `TBSTYLE_FLAT` (hot-tracking, no button borders until hovered) and
//!   `TBSTYLE_TOOLTIPS` (the toolbar creates a tooltip control for you;
//!   the texts themselves arrive via `WM_NOTIFY`/`TTN_GETDISPINFO` — a
//!   later chapter) — plus the same WM_SIZE-forwarding contract as e53
//!
//! New Win32 APIs: `TB_BUTTONSTRUCTSIZE`, `TB_LOADIMAGES`,
//! `IDB_STD_SMALL_COLOR`, `TB_ADDBUTTONSW`, `TB_AUTOSIZE`, `TBBUTTON`.
//!
//! Run: `cargo run -p e54_toolbar`

use windows::{Win32::*, core::*};

/// Command ids for the four buttons — deliberately in the 1000s range,
/// where menu ids live too: in a real app File>New on the menu bar and
/// the New toolbar button would share ONE id and ONE handler.
const IDC_FILENEW: i32 = 1001;
const IDC_FILEOPEN: i32 = 1002;
const IDC_FILESAVE: i32 = 1003;
const IDC_HELP: i32 = 1004;

#[derive(Default)]
struct State {
    hwnd_toolbar: HWND,
    actions: i32, // total actions performed, for the caption
}

fn main() {
    unsafe {
        // Toolbars register under ICC_BAR_CLASSES (with the trackbar and
        // status bar — e51/e53).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e54_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e54_class"),
            w!("e54 · Toolbar — click the standard buttons"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            260,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Step 1: create the control --------------------------------------
        // TOOLBARCLASSNAMEW is the exported w!("ToolbarWindow32") constant.
        // TBSTYLE_FLAT: borderless buttons that highlight on hover — the
        // look every app has had since Office 97. TBSTYLE_TOOLTIPS: the
        // toolbar builds an internal tooltip control; supplying the texts
        // happens through WM_NOTIFY/TTN_GETDISPINFO, which we don't handle
        // here (so no tooltip text is shown — the plumbing alone is the
        // point). CCS_TOP (docked at the top) is the default.
        let toolbar = CreateWindowExW(
            0,
            TOOLBARCLASSNAMEW,
            w!(""),
            (WS_CHILD | WS_VISIBLE | TBSTYLE_FLAT | TBSTYLE_TOOLTIPS) as u32,
            0,
            0,
            0,
            0, // the toolbar sizes itself
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!toolbar.is_null());

        // ---- Step 2: announce your TBBUTTON size -----------------------------
        // comctl32 has grown TBBUTTON-compatible fields across versions;
        // telling it your struct size lets old binaries talk to new DLLs.
        // This must come BEFORE adding any buttons.
        SendMessageW(
            toolbar,
            TB_BUTTONSTRUCTSIZE as u32,
            std::mem::size_of::<TBBUTTON>(),
            0,
        );

        // ---- Step 3: borrow the standard image strip ---------------------------
        // TB_LOADIMAGES with HINST_COMMCTRL (-1, a fake HINSTANCE meaning
        // "comctl32's own images") loads the built-in strip named by
        // IDB_STD_SMALL_COLOR: new/open/save/print/cut/copy/... The STD_*
        // constants below are indices INTO that strip.
        SendMessageW(
            toolbar,
            TB_LOADIMAGES as u32,
            IDB_STD_SMALL_COLOR as usize,
            HINST_COMMCTRL as isize,
        );

        // ---- Step 4: describe the buttons -------------------------------------
        // One TBBUTTON per button: which image, which command id, what
        // state, what style. iString/dwData/bReserved stay zero — no text
        // labels on the buttons themselves (add TBSTYLE_LIST to try that).
        let buttons = [
            tbutton(STD_FILENEW, IDC_FILENEW),
            tbutton(STD_FILEOPEN, IDC_FILEOPEN),
            tbutton(STD_FILESAVE, IDC_FILESAVE),
            tbutton(STD_HELP, IDC_HELP),
        ];
        SendMessageW(
            toolbar,
            TB_ADDBUTTONSW as u32,
            buttons.len(),
            buttons.as_ptr() as LPARAM,
        );
        // Ask the toolbar to fit itself to its content (the docs call this
        // mandatory after adding buttons when the bar manages its own size).
        SendMessageW(toolbar, TB_AUTOSIZE as u32, 0, 0);

        if let Some(s) = state_of(hwnd) {
            s.hwnd_toolbar = toolbar;
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// One plain enabled toolbar button: STD_* image + command id.
const fn tbutton(image: i32, command: i32) -> TBBUTTON {
    TBBUTTON {
        iBitmap: image,
        idCommand: command,
        fsState: TBSTATE_ENABLED as u8,
        fsStyle: BTNS_BUTTON as u8,
        // bReserved pads the struct: 6 bytes on 64-bit targets, 2 on x86.
        bReserved: [0; 6],
        dwData: 0,
        iString: 0,
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
            // Same channel as e41's BUTTON: notification in HIWORD (0 =
            // BN_CLICKED for toolbar buttons), command id in LOWORD. If a
            // menu bar existed, its items would land here too — same ids,
            // same arms.
            WM_COMMAND => {
                let control_id = (wparam & 0xffff) as u32;
                let what = match control_id as i32 {
                    IDC_FILENEW => "New",
                    IDC_FILEOPEN => "Open",
                    IDC_FILESAVE => "Save",
                    IDC_HELP => "Help",
                    _ => return 0,
                };
                state.actions += 1;
                _ = SetWindowTextW(
                    hwnd,
                    &HSTRING::from(format!(
                        "e54 · Toolbar — {what} ({} actions)",
                        state.actions
                    )),
                );
                0
            }
            // The e53 resize contract: toolbars re-dock on resize only if
            // the parent hands them its WM_SIZE.
            WM_SIZE => {
                SendMessageW(state.hwnd_toolbar, WM_SIZE as u32, wparam, lparam);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let line = to_wide(
                    "New · Open · Save · Help — standard images via \
                                   TB_LOADIMAGES, ids via WM_COMMAND",
                );
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    20,
                    80,
                    PCWSTR::from_raw(line.as_ptr()),
                    line.len() as i32 - 1,
                );
                _ = SetBkMode(hdc, OPAQUE);
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
