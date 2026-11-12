//! e70 — Splitter Window: a real child window you can drag
//!
//! What you learn:
//! * A splitter is not a control Windows gives you — you build it as a
//!   6px child window of a **custom class** whose class cursor is
//!   `IDC_SIZEWE` (the ↔ arrow). Because the cursor belongs to the class,
//!   Windows shows it for free whenever the mouse hovers the strip — no
//!   WM_SETCURSOR code needed.
//! * **Layout ownership**: the parent's WM_SIZE is the single place that
//!   positions all three children (left EDIT, splitter, right EDIT) via
//!   `SetWindowPos`. The splitter never moves itself — during a drag it
//!   only *reports* the desired split position to the parent with
//!   `SendMessageW`, and the parent re-runs the same layout code. One
//!   layout function, two triggers (resize + drag).
//! * The e13 mouse-capture technique, applied across windows:
//!   `SetCapture` on the splitter keeps WM_MOUSEMOVE flowing to it even
//!   when the pointer runs off the 6px strip, until WM_LBUTTONUP
//!   releases. Test with `GetCapture()` — no flags in sight.
//! * A custom message in the `WM_APP` range (`WM_APP + 1`) as the private
//!   parent↔child channel — values there are reserved for exactly this.
//!
//! Drag the gray strip: both panes follow. Resize the window: the split
//! survives, clamped to sane pane widths.
//!
//! New Win32 APIs: `IDC_SIZEWE`, `SetWindowPos`, `GetCapture`,
//! `MapWindowPoints`, `WM_APP`-range messages.
//!
//! Run: `cargo run -p e70_splitter_window`

use windows::{Win32::*, core::*};

/// Private "the splitter moved" message (WM_APP range = app-defined).
const WM_APP_SPLIT: i32 = WM_APP + 1;

/// Width of the splitter strip itself.
const SPLIT_W: i32 = 6;
/// No pane may shrink below this.
const MIN_PANE: i32 = 60;

#[derive(Default)]
struct State {
    /// x of the splitter's left edge in parent client coordinates — the
    /// one number the entire layout derives from.
    split_x: i32,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ---- The splitter's own class ---------------------------------
        // Its entire personality is the class cursor: hovering the strip
        // shows IDC_SIZEWE because the CLASS says so. Painting is the
        // class brush (COLOR_3DFACE = the standard dialog gray).
        let splitter_class = WNDCLASSW {
            lpfnWndProc: Some(splitter_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_SIZEWE.0.cast())),
            hbrBackground: ((COLOR_3DFACE + 1) as isize) as HBRUSH,
            lpszClassName: w!("e70_splitter"),
            ..Default::default() // no CS_HREDRAW: 6px windows don't need it
        };
        debug_assert!(RegisterClassW(&splitter_class) != 0);

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e70_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State { split_x: 320 }));

        let hwnd = CreateWindowExW(
            0,
            w!("e70_class"),
            w!("e70 · Splitter — drag the strip between the panes"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            420,
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

/// The single source of truth for geometry: position all three children
/// from `state.split_x`. Called from WM_SIZE (window resized) and from the
/// WM_APP_SPLIT report (splitter dragged) — the drag path is just "change
/// split_x, then pretend a resize happened".
unsafe fn layout(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let w = rc.right;

        // Clamp so neither pane can be squeezed out of existence.
        state.split_x = state
            .split_x
            .clamp(MIN_PANE, (w - SPLIT_W - MIN_PANE).max(MIN_PANE));

        // SetWindowPos over MoveWindow here because the splitter example
        // is the canonical place to meet it: handle + (x,y,cx,cy) + flags.
        // SWP_NOZORDER keeps the existing stacking (MoveWindow preserves
        // it too, but being explicit is the habit you want).
        if !state.split_x.is_negative() {
            _ = SetWindowPos(
                GetDlgItem(Some(hwnd), 1001), // left EDIT
                None,
                0,
                0,
                state.split_x,
                rc.bottom,
                SWP_NOZORDER as u32,
            );
            _ = SetWindowPos(
                GetDlgItem(Some(hwnd), 1002), // the splitter
                None,
                state.split_x,
                0,
                SPLIT_W,
                rc.bottom,
                SWP_NOZORDER as u32,
            );
            _ = SetWindowPos(
                GetDlgItem(Some(hwnd), 1003), // right EDIT
                None,
                state.split_x + SPLIT_W,
                0,
                w - state.split_x - SPLIT_W,
                rc.bottom,
                SWP_NOZORDER as u32,
            );
        }
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
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;
                let edit_style = (WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | ES_MULTILINE
                    | ES_READONLY
                    | ES_AUTOVSCROLL) as u32;

                let left = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    edit_style,
                    0,
                    0,
                    0,
                    0, // sized by layout()
                    Some(hwnd),
                    Some(1001usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!left.is_null());
                let right = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    edit_style,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(1003usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!right.is_null());

                let splitter = CreateWindowExW(
                    0,
                    w!("e70_splitter"), // our class, not "BUTTON"/"EDIT"
                    w!(""),
                    (WS_CHILD | WS_VISIBLE) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(1002usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!splitter.is_null());
                let _ = splitter;

                // Content for the panes (EDIT wants CRLF line breaks).
                let lorem = "The splitter is an ordinary child window.\r\n\
                             It never resizes itself.\r\n\
                             During a drag it just tells the parent where it wants to be,\r\n\
                             and the parent runs the same layout as on WM_SIZE.\r\n";
                _ = SetWindowTextW(left, &HSTRING::from(lorem));
                let ipsum = "Layout ownership lives in ONE place:\r\n\
                             the parent's layout() function.\r\n\
                             Resize the window, then drag the strip -\r\n\
                             both paths funnel into the same code.\r\n";
                _ = SetWindowTextW(right, &HSTRING::from(ipsum));

                layout(hwnd);
                0
            }

            WM_SIZE => {
                layout(hwnd);
                0
            }

            // The splitter's drag report: wparam = desired split_x in
            // parent client coordinates. Update state, run the layout.
            WM_APP_SPLIT => {
                if let Some(state) = state_of(hwnd) {
                    state.split_x = wparam as i32;
                }
                layout(hwnd);
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
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

// ======================================================================
// The splitter child window
// ======================================================================
extern "system" fn splitter_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            // Leash the mouse (e13 technique): from here until
            // WM_LBUTTONUP every mouse message — even far outside this
            // 6-pixel strip — is delivered to US.
            WM_LBUTTONDOWN => {
                _ = SetCapture(hwnd);
                0
            }

            WM_MOUSEMOVE => {
                // Dragging = "we hold the capture". GetCapture() is the
                // stateless way to ask; no bool needed anywhere.
                if GetCapture() == hwnd {
                    let parent = GetParent(hwnd);
                    if !parent.is_null() {
                        // The splitter's own client x is useless (0..6) —
                        // we need the pointer in PARENT coordinates.
                        // Screen coordinates are the neutral ground: take
                        // the cursor position, then map it into the parent.
                        let mut pt = POINT { x: 0, y: 0 };
                        _ = GetCursorPos(&mut pt);
                        _ = MapWindowPoints(None, Some(parent), &mut pt, 1);
                        // (MapWindowPoints(None, ...) means "from screen".)

                        // Hand the request to the parent — SEND, not POST:
                        // layout lands synchronously, so the panes track
                        // the cursor with no lag or queue pile-up.
                        SendMessageW(parent, WM_APP_SPLIT as u32, pt.x as WPARAM, 0);
                    }
                }
                0
            }

            WM_LBUTTONUP => {
                if GetCapture() == hwnd {
                    _ = ReleaseCapture();
                }
                0
            }

            WM_CAPTURECHANGED => {
                // The system yanked the capture (a modal dialog appeared,
                // another window grabbed it...). Nothing to clean up since
                // we keep no drag state — this arm exists to document that
                // the stateless design is why that is okay.
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
