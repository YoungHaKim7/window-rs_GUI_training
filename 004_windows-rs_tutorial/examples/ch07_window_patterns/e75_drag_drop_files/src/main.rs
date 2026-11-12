//! e75 — Drag & Drop Files: WM_DROPFILES, the 1995 way (still the easy way)
//!
//! What you learn:
//! * `DragAcceptFiles(hwnd, TRUE)` — one call at creation opts the window
//!   into SHELL file drops: Explorer's drag cursor changes over it, and
//!   dropping posts a single `WM_DROPFILES` whose **wparam is an HDROP**
//!   — an opaque handle to the dropped-file list.
//! * The three-query dance on that handle:
//!   `DragQueryFileW(hdrop, 0xFFFFFFFF, None, 0)` → file COUNT;
//!   `DragQueryFileW(hdrop, i, None, 0)` → file i's length;
//!   `DragQueryFileW(hdrop, i, Some(buf), len)` → the path itself.
//!   Two lessons in one: the i32::MAX index is the "just count" idiom,
//!   and the None-buffer call is the classic query-then-fetch pattern
//!   Win32 uses wherever a string's size is unknown up front.
//! * `DragFinish` — releases the HDROP's memory. Forgetting it leaks on
//!   every drop; there is no automatic cleanup.
//! * The API's limits, honestly: WM_DROPFILES works from Explorer only
//!   (not between your own windows), can't tell clicks from cancels,
//!   and predates long paths nicely. The modern answer is OLE's
//!   `IDropTarget` (RegisterDragDrop + a COM object) — more machinery,
//!   but it covers in-app drags, feedback cursors, and non-file data.
//!   For "user drops files from Explorer", WM_DROPFILES is still the
//!   five-line solution. (e05 showed the per-window style flag
//!   WS_EX_ACCEPTFILES — same feature, opt-in via CreateWindowExW instead
//!   of the explicit call used here.)
//!
//! Drag files from Explorer onto this window — the paths paint below.
//!
//! New Win32 APIs: `DragAcceptFiles`, `WM_DROPFILES`, `DragQueryFileW`,
//! `DragFinish` (shellapi).
//!
//! Run: `cargo run -p e75_drag_drop_files`

use windows::{Win32::*, core::*};

/// Paint at most this many paths, then a "+N more" line.
const MAX_SHOWN: usize = 10;

#[derive(Default)]
struct State {
    paths: Vec<String>,
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
            lpszClassName: w!("e75_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e75_class"),
            w!("e75 · Drag & Drop — drop files from Explorer here"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            680,
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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
                // The whole opt-in. TRUE (the bool, not a BOOL) flips the
                // window's internal WS_EX_ACCEPTFILES bit on — the same
                // flag e05 set through CreateWindowExW's exStyle.
                DragAcceptFiles(hwnd, true);
                0
            }

            // wparam = HDROP, an opaque shell handle. Note what it is NOT:
            // not a file list pointer, not global memory you may read —
            // DragQueryFileW is the only legal accessor.
            WM_DROPFILES => {
                let hdrop = wparam as HDROP;

                // Query 1 — the count: index 0xFFFFFFFF means "no file,
                // just tell me how many". (The C docs spell it
                // 0xFFFFFFFF; any i32::MAX-style sentinel works the same.)
                let count = DragQueryFileW(hdrop, u32::MAX, None, 0);

                let mut paths = Vec::with_capacity(count as usize);
                for i in 0..count {
                    // Query 2 — file i's length, in characters, WITHOUT a
                    // terminator. Passing no buffer turns the call into a
                    // size query (the same trick GetTextExtent-family
                    // APIs use).
                    let len = DragQueryFileW(hdrop, i, None, 0) as usize;

                    // Query 3 — the path itself, into a len+1 buffer.
                    let mut buf = vec![0u16; len + 1];
                    let copied =
                        DragQueryFileW(hdrop, i, Some(PWSTR(buf.as_mut_ptr())), buf.len() as u32);
                    debug_assert!(copied as usize == len);

                    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
                    paths.push(String::from_utf16_lossy(&buf[..end]));
                }

                // Release the handle's memory — once, exactly here. The
                // HDROP is dead after this call.
                DragFinish(hdrop);

                if let Some(state) = state_of(hwnd) {
                    state.paths = paths;
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                _ = SetBkMode(hdc, TRANSPARENT);

                let Some(state) = state_of(hwnd) else {
                    _ = EndPaint(hwnd, &ps);
                    return 0;
                };

                if state.paths.is_empty() {
                    _ = SetTextColor(hdc, rgb(0x80, 0x80, 0x80));
                    let wide = to_wide("drop files from an Explorer window onto this one");
                    let mut area = rc;
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut area,
                        (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                    );
                } else {
                    _ = SetTextColor(hdc, rgb(0x20, 0x20, 0x20));
                    let header = to_wide(&format!("{} file(s) dropped:", state.paths.len()));
                    _ = TextOutW(hdc, 16, 14, PCWSTR::from_raw(header.as_ptr()), -1);

                    for (i, path) in state.paths.iter().take(MAX_SHOWN).enumerate() {
                        let wide = to_wide(path);
                        _ = TextOutW(
                            hdc,
                            28,
                            40 + i as i32 * 20,
                            PCWSTR::from_raw(wide.as_ptr()),
                            -1,
                        );
                    }
                    if state.paths.len() > MAX_SHOWN {
                        let more =
                            to_wide(&format!("... and {} more", state.paths.len() - MAX_SHOWN));
                        _ = SetTextColor(hdc, rgb(0x80, 0x80, 0x80));
                        _ = TextOutW(
                            hdc,
                            28,
                            40 + MAX_SHOWN as i32 * 20,
                            PCWSTR::from_raw(more.as_ptr()),
                            -1,
                        );
                    }
                }

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
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
