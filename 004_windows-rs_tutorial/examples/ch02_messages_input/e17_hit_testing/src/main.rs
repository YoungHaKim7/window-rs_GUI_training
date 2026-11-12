//! e17 — Hit Testing: a frameless window you can still drag
//!
//! What you learn:
//! * `WM_NCHITTEST` — before every mouse action Windows asks the window
//!   "what part of you is under this point?" The answer (an HT* code)
//!   decides what the click *means*: client area, title bar, border...
//! * Returning `HTCAPTION` for your own painted strip makes Windows drag
//!   the window for you — the whole move loop, for free
//! * `WS_POPUP`: a window with no frame at all — you draw everything
//! * `WM_NCHITTEST` coordinates are in **screen** space, unlike the client
//!   coordinates of ordinary mouse messages: `ScreenToClient` converts
//!
//! Drag the colored strip to move the window. Click the X box (or press
//! Esc) to close — there is no real title bar to do it for you.
//!
//! New Win32 APIs: `WM_NCHITTEST`, `ScreenToClient`, `HTCAPTION`/`HTCLIENT`.
//!
//! Run: `cargo run -p e17_hit_testing`

use windows::{Win32::*, core::*};

const CAPTION_HEIGHT: i32 = 40; // our hand-painted title strip
const CLOSE_WIDTH: i32 = 48; // the X box at its right end

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e17_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // WS_POPUP = no caption, no border, no system menu. (It happens to
        // be a u32 constant in this crate, unlike most WS_* values.)
        let hwnd = CreateWindowExW(
            0,
            w!("e17_class"),
            w!("e17 · Hit Testing"),
            WS_POPUP | WS_VISIBLE as u32,
            240,
            200,
            560,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

/// Is this client-area point inside our painted close box?
unsafe fn in_close_box(hwnd: HWND, x: i32, y: i32) -> bool {
    unsafe {
        let mut rect = RECT::default();
        _ = GetClientRect(hwnd, &mut rect);
        y < CAPTION_HEIGHT && x >= rect.right - CLOSE_WIDTH
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCHITTEST => {
                // lparam holds SCREEN coordinates here. Convert to client
                // space so we can compare against our own layout.
                let (sx, sy) = mouse_pos(lparam);
                let mut point = POINT { x: sx, y: sy };
                _ = ScreenToClient(hwnd, &mut point);

                if in_close_box(hwnd, point.x, point.y) {
                    // Report "client area" so the click reaches us as a
                    // normal WM_LBUTTONUP below.
                    HTCLIENT as LRESULT
                } else if point.y < CAPTION_HEIGHT {
                    // The lie that makes it all work: claim this strip is a
                    // title bar. Windows then runs its standard drag loop.
                    HTCAPTION as LRESULT
                } else {
                    HTCLIENT as LRESULT
                }
            }
            WM_LBUTTONUP => {
                let (x, y) = mouse_pos(lparam);
                if in_close_box(hwnd, x, y) {
                    _ = DestroyWindow(hwnd);
                }
                0
            }
            WM_KEYDOWN => {
                if wparam as i32 == VK_ESCAPE {
                    _ = DestroyWindow(hwnd);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // The fake title strip.
                let strip = RECT {
                    bottom: CAPTION_HEIGHT,
                    ..rect
                };
                FillRect(hdc, &strip, GetSysColorBrush(COLOR_HIGHLIGHT));

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, 0x00FF_FFFF); // white (0x00BBGGRR)
                _ = TextOutW(hdc, 14, 11, w!("Drag me — I answer HTCAPTION"), -1);

                // The close box: a darker patch with an X.
                let close = RECT {
                    left: rect.right - CLOSE_WIDTH,
                    top: 0,
                    right: rect.right,
                    bottom: CAPTION_HEIGHT,
                };
                FillRect(hdc, &close, GetSysColorBrush(COLOR_WINDOWTEXT));
                _ = TextOutW(hdc, close.left + 19, 11, w!("X"), -1);

                // Body text, plus a 1px frame since WS_POPUP has no border.
                _ = SetTextColor(hdc, 0x0000_0000);
                _ = TextOutW(
                    hdc,
                    14,
                    CAPTION_HEIGHT + 20,
                    w!("The area below the strip answers HTCLIENT: no dragging here."),
                    -1,
                );
                _ = TextOutW(
                    hdc,
                    14,
                    CAPTION_HEIGHT + 48,
                    w!("Esc or the X box closes the window."),
                    -1,
                );
                FrameRect(hdc, &rect, GetSysColorBrush(COLOR_WINDOWTEXT));

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
