//! e90 — Screen Capture: the whole desktop, cached, saveable
//!
//! What you learn:
//! * `GetDC(None)` — the device context of the *entire virtual screen*; from
//!   it you can BitBlt anything on any monitor (primary monitor's
//!   SM_CXSCREEN × SM_CYSCREEN here)
//! * The **cached capture** pattern: the screen snapshot lives in one
//!   `CreateCompatibleBitmap` owned by the window state, taken once at
//!   startup and re-taken on key R. WM_PAINT only *plays it back* — GDI
//!   never touches the live screen while you expose
//! * `StretchBlt` — BitBlt's scaling cousin, used to fit the (huge) screen
//!   into the (small) client area
//! * The e32 .bmp writer, reused verbatim: `GetDIBits` for the raw pixels,
//!   then the field-by-field `BITMAPFILEHEADER` dance (packed to 14 bytes on
//!   disk, 16 in the Rust struct) — press S to save `screen_capture.bmp`
//!
//! The DPI caveat: this app is not DPI-aware, so on a scaled display (say
//! 150%) Windows reports and hands out *virtualized* coordinates and
//! bitmaps — your capture may be blurrier or a different pixel size than
//! the physical panel. Real screenshot tools call SetProcessDpiAwareness-
//! Context first (and usually graduate from GDI to DXGI Desktop Duplication).
//!
//! Press R to re-capture, S to save the current capture as .bmp.
//!
//! New Win32 APIs: `StretchBlt` (wingdi); `GetDC(None)`, `GetSystemMetrics`,
//! `SM_CXSCREEN`/`SM_CYSCREEN` (winuser) — the rest you met in e32.
//!
//! Run: `cargo run -p e90_screen_capture`

use std::io::Write as _;
use windows::{Win32::*, core::*};

#[derive(Default)]
struct State {
    /// The cached screen snapshot. Recreated on each capture; drawn on every
    /// WM_PAINT. Owned HBITMAP — deleted when replaced or at teardown.
    hbmp: HBITMAP,
    size: (i32, i32), // pixel size of the capture
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
            lpszClassName: w!("e90_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e90_class"),
            w!("e90 · Screen Capture — R = recapture, S = save .bmp"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 800, 500,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // One capture at startup so the window opens already showing the
        // desktop behind it (this window included — the screen DC sees all).
        if let Some(s) = state_of(hwnd) {
            capture_screen(s);
        }
        _ = InvalidateRect(Some(hwnd), None, true);

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

/// Snapshot the primary screen into state's cached bitmap (e32's chain,
/// pointed at GetDC(None) instead of a window).
unsafe fn capture_screen(state: &mut State) {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);

        let screen = GetDC(None); // None = the whole screen's DC
        let mem = CreateCompatibleDC(Some(screen));
        // Compatible with the SCREEN dc — the classic e32 lesson.
        let hbmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, hbmp as HGDIOBJ);
        // (0,0) is the primary monitor's top-left. Multi-monitor setups:
        // the virtual desktop is bigger, and starts at SM_XVIRTUALSCREEN.
        _ = BitBlt(mem, 0, 0, w, h, Some(screen), 0, 0, SRCCOPY);
        SelectObject(mem, old);

        // Swap the capture in, retiring (deleting) any previous one.
        if !state.hbmp.is_null() {
            _ = DeleteObject(state.hbmp as HGDIOBJ);
        }
        state.hbmp = hbmp;
        state.size = (w, h);

        _ = DeleteDC(mem);
        _ = ReleaseDC(None, screen);
    }
}

/// Write the cached capture to a 24-bpp .bmp next to the exe — e32's writer,
/// lifted as-is (GetDIBits needs the bitmap selected into a DC first).
unsafe fn save_capture(state: &State) -> std::io::Result<std::path::PathBuf> {
    unsafe {
        let (w, h) = state.size;

        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let old = SelectObject(mem, state.hbmp as HGDIOBJ);

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h; // positive = bottom-up rows
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 24; // 3 bytes/pixel, BGR, no palette
        bmi.bmiHeader.biCompression = BI_RGB as u32;

        let stride = (w as usize * 3 + 3) & !3; // rows padded to 4 bytes
        let mut pixels = vec![0u8; stride * h as usize];
        let copied = GetDIBits(
            mem,
            state.hbmp,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as LPVOID),
            &mut bmi,
            DIB_RGB_COLORS as u32,
        );
        debug_assert!(copied == h);

        SelectObject(mem, old);
        _ = DeleteDC(mem);
        _ = ReleaseDC(None, screen);

        // BITMAPFILEHEADER: packed to 14 bytes on disk; the Rust struct is
        // 16 (repr(C) alignment) — so field by field, never a struct blit.
        let mut file: Vec<u8> = Vec::with_capacity(54 + pixels.len());
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());

        // BITMAPINFOHEADER: exactly 40 bytes, no hidden padding.
        debug_assert_eq!(std::mem::size_of::<BITMAPINFOHEADER>(), 40);
        let header = &bmi.bmiHeader as *const _ as *const u8;
        file.extend_from_slice(std::slice::from_raw_parts(header, 40));
        file.extend_from_slice(&pixels);

        let exe = std::env::current_exe()?;
        let path = exe
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default()
            .join("screen_capture.bmp");
        let mut f = std::fs::File::create(&path)?;
        f.write_all(&file)?;
        f.flush()?;
        Ok(path)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_KEYDOWN => {
                if wparam as i32 == 'R' as i32 {
                    capture_screen(state);
                    _ = InvalidateRect(Some(hwnd), None, true);
                    0
                } else if wparam as i32 == 'S' as i32 {
                    match save_capture(state) {
                        Ok(path) => {
                            _ = SetWindowTextW(
                                hwnd,
                                &HSTRING::from(format!(
                                    "e90 · saved {} ({}x{} px)",
                                    path.display(), state.size.0, state.size.1
                                )),
                            );
                        }
                        Err(e) => println!("save failed: {e}"),
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

                if !state.hbmp.is_null() {
                    // Playback only: the cached bitmap goes into a scratch
                    // memory DC, then StretchBlt scales it to fit the
                    // client. (BitBlt would draw it 1:1 and clip — fine
                    // for same-size targets, useless for a 1920x1080
                    // snapshot in an 800x500 window.)
                    let mem = CreateCompatibleDC(Some(hdc));
                    let old = SelectObject(mem, state.hbmp as HGDIOBJ);
                    _ = StretchBlt(
                        hdc,
                        0, 0, rc.right, rc.bottom,
                        Some(mem),
                        0, 0, state.size.0, state.size.1,
                        SRCCOPY,
                    );
                    SelectObject(hdc, old);
                    _ = DeleteDC(mem);
                }

                _ = SetBkMode(hdc, TRANSPARENT);
                let hint = to_wide("R = recapture · S = save screen_capture.bmp");
                _ = TextOutW(hdc, 8, 8, PCWSTR::from_raw(hint.as_ptr()), hint.len() as i32 - 1);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                if !state.hbmp.is_null() {
                    _ = DeleteObject(state.hbmp as HGDIOBJ);
                }
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
