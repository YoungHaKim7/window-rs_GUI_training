//! e83 — Paint App: strokes, menu-driven settings, save to .bmp
//!
//! What you learn:
//! * **The stroke model** — a drawing is `Vec<Stroke>`, a stroke is
//!   `Vec<POINT>` plus the pen it was drawn with. WM_PAINT replays the
//!   whole vector every time (the e12→e13 lesson at application scale:
//!   the model persists, the screen is disposable).
//! * **Right-drag as eraser** — no erasing at all, really: an eraser
//!   stroke is an ordinary stroke drawn in the BACKGROUND color with a
//!   fat pen. Undo-friendly and pixel-perfect; this is also how most
//!   simple paint programs implement "erase".
//! * Menu-driven settings with state: color items are radio-checked
//!   (`CheckMenuRadioItem`), pen widths a submenu of radio items — the
//!   menu itself becomes a visible control (e35's territory).
//! * Save As BMP reuses e32's recipe verbatim: capture client →
//!   `GetDIBits` 24-bpp bottom-up → write `BITMAPFILEHEADER`
//!   **field-by-field** (it is packed to 14 bytes on disk while repr(C)
//!   pads it to 16 — never blit that struct) + the 40-byte
//!   BITMAPINFOHEADER + padded BGR rows.
//!
//! Draw with the left button, erase with the right; change color/width
//! in the Draw menu; Clear asks first; Save As BMP writes next to the exe.
//!
//! Run: `cargo run -p e83_paint_app`

use std::io::Write as _;
use windows::{Win32::*, core::*};

const IDM_COLOR_BASE: usize = 5100; // 5100..5103 = red/green/blue/black
const IDM_WIDTH_BASE: usize = 5200; // 5201/5203/5206 = 1/3/6 px
const IDM_CLEAR: usize = 5300;
const IDM_SAVE: usize = 5301;

const PALETTE: [(u8, u8, u8); 4] = [(0xC0, 0x30, 0x30), (0x30, 0x90, 0x40), (0x30, 0x50, 0xC0), (0x20, 0x20, 0x20)];
const WIDTHS: [i32; 3] = [1, 3, 6];
const BACKGROUND: COLORREF = 0x00FF_FFFF; // the paper: white

/// One committed or in-flight polyline with its own pen.
struct Stroke {
    pts: Vec<POINT>,
    color: COLORREF,
    width: i32,
}

struct State {
    strokes: Vec<Stroke>,
    /// The stroke under the mouse right now (committed on button-up).
    current: Option<Stroke>,
    color_index: usize,
    width_index: usize,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_CROSS.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e83_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State {
            strokes: Vec::new(),
            current: None,
            color_index: 3, // black
            width_index: 1, // 3 px
        }));

        let hwnd = CreateWindowExW(
            0,
            w!("e83_class"),
            w!("e83 · Paint — left draws, right erases"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 720, 520,
            None, None, Some(instance),
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

fn mouse_pos(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam & 0xffff) as i16 as i32,
        y: ((lparam >> 16) & 0xffff) as i16 as i32,
    }
}

unsafe fn draw_stroke(hdc: HDC, stroke: &Stroke) {
    unsafe {
        if stroke.pts.len() < 2 {
            return;
        }
        let pen = CreatePen(PS_SOLID, stroke.width, stroke.color);
        let old = SelectObject(hdc, pen as HGDIOBJ);
        _ = MoveToEx(hdc, stroke.pts[0].x, stroke.pts[0].y, None);
        for pt in &stroke.pts[1..] {
            _ = LineTo(hdc, pt.x, pt.y);
        }
        SelectObject(hdc, old);
        _ = DeleteObject(pen as HGDIOBJ);
    }
}

// ======================================================================
// The .bmp writer — e32's recipe, reused verbatim (self-contained by
// design: no shared helper crate in this tutorial).
// ======================================================================
unsafe fn save_client_bitmap(hwnd: HWND) -> std::io::Result<std::path::PathBuf> {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let (w, h) = (rc.right, rc.bottom);
        debug_assert!(w > 0 && h > 0);

        // 1. Blit the client pixels into a memory bitmap.
        let screen = GetDC(Some(hwnd));
        let mem = CreateCompatibleDC(Some(screen));
        // Compatible with the SCREEN dc, not the memory dc (the classic
        // CreateCompatibleBitmap trap — else you get 1x1 monochrome).
        let hbmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, hbmp as HGDIOBJ);
        _ = BitBlt(mem, 0, 0, w, h, Some(screen), 0, 0, SRCCOPY);

        // 2. GetDIBits: 24 bpp, bottom-up, BGR.
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h; // positive = bottom-up rows
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 24;
        bmi.bmiHeader.biCompression = BI_RGB as u32;

        let stride = (w as usize * 3 + 3) & !3; // rows padded to 4 bytes
        let mut pixels = vec![0u8; stride * h as usize];
        let copied = GetDIBits(
            mem,
            hbmp,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as LPVOID),
            &mut bmi,
            DIB_RGB_COLORS as u32,
        );
        debug_assert!(copied == h);

        SelectObject(mem, old);
        _ = DeleteObject(hbmp as HGDIOBJ);
        _ = DeleteDC(mem);
        _ = ReleaseDC(Some(hwnd), screen);

        // 3. Assemble the file. FILEHEADER(14) + INFOHEADER(40) + rows.
        let pixel_bytes = pixels.len() as u32;
        let mut file: Vec<u8> = Vec::with_capacity(54 + pixel_bytes as usize);

        // BITMAPFILEHEADER is 14 bytes PACKED on disk; the Rust struct is
        // 16 (alignment padding after the WORD bfType) — serialize it
        // field by field, never as a struct blit.
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(54 + pixel_bytes).to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());

        // BITMAPINFOHEADER is exactly 40 bytes, no hidden padding.
        debug_assert_eq!(std::mem::size_of::<BITMAPINFOHEADER>(), 40);
        let header = &bmi.bmiHeader as *const _ as *const u8;
        file.extend_from_slice(std::slice::from_raw_parts(header, 40));

        // GetDIBits already wrote bottom-up padded BGR rows.
        file.extend_from_slice(&pixels);

        // 4. Write next to the exe.
        let exe = std::env::current_exe()?;
        let path = exe
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default()
            .join("painting.bmp");
        let mut f = std::fs::File::create(&path)?;
        f.write_all(&file)?;
        f.flush()?;
        Ok(path)
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
                let draw_menu = CreatePopupMenu();
                for (i, name) in ["Red", "Green", "Blue", "Black"].iter().enumerate() {
                    let label = format!("&{name}");
                    let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                    _ = AppendMenuW(
                        draw_menu,
                        MF_STRING as u32,
                        IDM_COLOR_BASE + i,
                        PCWSTR::from_raw(wide.as_ptr()),
                    );
                }

                // Width submenu: radio items 1/3/6.
                let width_menu = CreatePopupMenu();
                for (i, w) in WIDTHS.iter().enumerate() {
                    let label = if *w == 1 { "&Thin (1 px)" } else if *w == 3 { "&Medium (3 px)" } else { "&Thick (6 px)" };
                    let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                    _ = AppendMenuW(
                        width_menu,
                        MF_STRING as u32,
                        IDM_WIDTH_BASE + *w as usize,
                        PCWSTR::from_raw(wide.as_ptr()),
                    );
                    let _ = i;
                }

                _ = AppendMenuW(draw_menu, MF_SEPARATOR as u32, 0, w!(""));
                _ = AppendMenuW(draw_menu, MF_POPUP as u32, width_menu as usize, w!("&Width"));
                _ = AppendMenuW(draw_menu, MF_SEPARATOR as u32, 0, w!(""));
                _ = AppendMenuW(draw_menu, MF_STRING as u32, IDM_CLEAR, w!("&Clear..."));
                _ = AppendMenuW(draw_menu, MF_STRING as u32, IDM_SAVE, w!("Save as &BMP..."));

                let bar = CreateMenu();
                _ = AppendMenuW(bar, MF_POPUP as u32, draw_menu as usize, w!("&Draw"));
                _ = SetMenu(hwnd, Some(bar));

                // Initial check marks: the menu mirrors state.
                let _ = CheckMenuRadioItem(
                    draw_menu,
                    (IDM_COLOR_BASE) as u32,
                    (IDM_COLOR_BASE + 3) as u32,
                    (IDM_COLOR_BASE + 3) as u32, // black
                    MF_BYCOMMAND as u32,
                );
                let _ = CheckMenuRadioItem(
                    width_menu,
                    (IDM_WIDTH_BASE + 1) as u32,
                    (IDM_WIDTH_BASE + 6) as u32,
                    (IDM_WIDTH_BASE + 3) as u32, // 3 px
                    MF_BYCOMMAND as u32,
                );
                0
            }

            WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
                // Right button = eraser: same stroke machinery, fat pen
                // in the paper's color.
                _ = SetCapture(hwnd);
                if let Some(state) = state_of(hwnd) {
                    let eraser = message as i32 == WM_RBUTTONDOWN;
                    let (r, g, b) = PALETTE[state.color_index];
                    state.current = Some(Stroke {
                        pts: vec![mouse_pos(lparam)],
                        color: if eraser { BACKGROUND } else { rgb(r, g, b) },
                        width: if eraser { 14 } else { WIDTHS[state.width_index] },
                    });
                }
                0
            }

            WM_MOUSEMOVE => {
                if GetCapture() == hwnd {
                    if let Some(state) = state_of(hwnd) {
                        if let Some(stroke) = state.current.as_mut() {
                            stroke.pts.push(mouse_pos(lparam));
                        }
                    }
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }

            WM_LBUTTONUP | WM_RBUTTONUP => {
                if GetCapture() == hwnd {
                    _ = ReleaseCapture();
                    if let Some(state) = state_of(hwnd) {
                        if let Some(stroke) = state.current.take() {
                            if stroke.pts.len() >= 2 {
                                state.strokes.push(stroke);
                            }
                        }
                    }
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }

            WM_COMMAND => {
                let id = (wparam & 0xffff) as usize;
                match id {
                    // Colors: update state, move the radio dot.
                    cid if cid >= IDM_COLOR_BASE && cid < IDM_COLOR_BASE + PALETTE.len() => {
                        if let Some(state) = state_of(hwnd) {
                            state.color_index = cid - IDM_COLOR_BASE;
                        }
                        let bar = GetMenu(hwnd);
                        let draw_menu = GetSubMenu(bar, 0);
                        let _ = CheckMenuRadioItem(
                            draw_menu,
                            IDM_COLOR_BASE as u32,
                            (IDM_COLOR_BASE + 3) as u32,
                            cid as u32,
                            MF_BYCOMMAND as u32,
                        );
                        0
                    }

                    // Widths: ids encode the pixel count (5201/5203/5206).
                    wid if wid > IDM_WIDTH_BASE && wid <= IDM_WIDTH_BASE + 6 => {
                        if let Some(state) = state_of(hwnd) {
                            let px = (wid - IDM_WIDTH_BASE) as i32;
                            if let Some(idx) = WIDTHS.iter().position(|&w| w == px) {
                                state.width_index = idx;
                                // Radio the width submenu (Draw > Width).
                                let bar = GetMenu(hwnd);
                                let draw_menu = GetSubMenu(bar, 0);
                                let width_menu = GetSubMenu(draw_menu, 1);
                                let _ = CheckMenuRadioItem(
                                    width_menu,
                                    (IDM_WIDTH_BASE + 1) as u32,
                                    (IDM_WIDTH_BASE + 6) as u32,
                                    wid as u32,
                                    MF_BYCOMMAND as u32,
                                );
                            }
                        }
                        0
                    }

                    IDM_CLEAR => {
                        // Destructive action → confirm (borrow released
                        // before the MessageBox loop, e65 rule).
                        let has_art = state_of(hwnd).map(|s| !s.strokes.is_empty()).unwrap_or(false);
                        let confirmed = !has_art
                            || MessageBoxW(
                                Some(hwnd),
                                w!("Throw away the whole drawing?"),
                                w!("e83 Paint"),
                                (MB_OKCANCEL | MB_ICONQUESTION) as u32,
                            ) == IDOK;
                        if confirmed {
                            if let Some(state) = state_of(hwnd) {
                                state.strokes.clear();
                            }
                            _ = InvalidateRect(Some(hwnd), None, true);
                        }
                        0
                    }

                    IDM_SAVE => {
                        match save_client_bitmap(hwnd) {
                            Ok(path) => {
                                _ = SetWindowTextW(
                                    hwnd,
                                    &HSTRING::from(format!("e83 · saved {}", path.display())),
                                );
                            }
                            Err(err) => {
                                _ = MessageBoxW(
                                    Some(hwnd),
                                    &HSTRING::from(format!("Save failed:\n{err}")),
                                    w!("e83 Paint"),
                                    (MB_OK | MB_ICONERROR) as u32,
                                );
                            }
                        }
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

                // The paper. WM_ERASEBKGND-free approach: paint it here
                // every frame so eraser strokes blend with the surface.
                let paper = CreateSolidBrush(BACKGROUND);
                _ = FillRect(hdc, &rc, paper);
                _ = DeleteObject(paper as HGDIOBJ);

                if let Some(state) = state_of(hwnd) {
                    for stroke in state.strokes.iter() {
                        draw_stroke(hdc, stroke);
                    }
                    if let Some(stroke) = state.current.as_ref() {
                        draw_stroke(hdc, stroke);
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
