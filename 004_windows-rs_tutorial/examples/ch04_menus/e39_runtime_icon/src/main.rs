//! e39 — Runtime Icon: a 32×32 icon generated pixel by pixel in code
//!
//! What you learn:
//! * How an icon is *actually built*: two bitmaps handed to
//!   `CreateIconIndirect` via `ICONINFO` — a color **XOR plane** (here
//!   32 bpp, 4 bytes/pixel BGRA) and a monochrome **AND mask** (1 bpp,
//!   1 = transparent) layered underneath. Wherever the mask bit is set,
//!   the pixel is transparent no matter what the color plane says
//! * `CreateBitmap` — the raw "here is an array of bits, give me an
//!   HBITMAP" call (no DIB headers, no file, just pixels; 1-bpp rows are
//!   padded to 32 bits — 32 px wide is exactly 4 bytes, so no padding
//!   kicks in here)
//! * `SendMessageW(WM_SETICON, ICON_SMALL / ICON_BIG)` — swap a window's
//!   title-bar and taskbar icons *while it runs*. The window does NOT
//!   take ownership: keep the HICON and `DestroyIcon` it yourself
//! * The client area repaints the *same* pixels (one 8×8 block per icon
//!   pixel) from the same `icon_pixel` function — what you see big is
//!   literally what the title bar shows small
//! * For real .ico files you'd use `LoadImageW(None, path, IMAGE_ICON,
//!   cx, cy, LR_LOADFROMFILE)` instead — this example builds one in code
//!   only to show what an icon contains
//!
//! Watch the title bar (and Alt+Tab): the icon changes moments after the
//! window appears.
//!
//! New Win32 APIs: `CreateBitmap`, `CreateIconIndirect`, `WM_SETICON`,
//! `DestroyIcon` (winuser/wingdi).
//!
//! Run: `cargo run -p e39_runtime_icon`

use windows::{Win32::*, core::*};

const SIZE: i32 = 32; // icon dimensions (px)

#[derive(Default)]
struct State {
    icon: Option<HICON>, // kept alive so WM_DESTROY can free it
}

/// The single source of truth for the icon's look: pixel (x, y) → Some
/// (red, green, blue) or None for transparent. Used to build BOTH the
/// icon planes and the big client-area rendering.
fn icon_pixel(x: i32, y: i32) -> Option<(u8, u8, u8)> {
    let (cx, cy) = (15.5_f32, 15.5_f32); // disc center in pixel space
    let dx = x as f32 - cx;
    let dy = y as f32 - cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist > 14.0 {
        None // outside the disc → transparent (mask bit = 1)
    } else if dist > 11.5 {
        Some((0x1E, 0x3A, 0x8D)) // dark blue ring
    } else if (9..=12).contains(&x) && (9..=12).contains(&y) {
        Some((0xFF, 0xFF, 0xFF)) // white highlight, upper-left
    } else {
        Some((0xE8, 0x88, 0x30)) // orange disc body
    }
}

/// Build the two bitmaps, combine them into an HICON. The caller owns
/// the returned icon (and we delete the bitmaps ourselves — the icon
/// copies what it needs at creation time).
unsafe fn build_icon() -> HICON {
    unsafe {
        // ---- Color (XOR) plane: 32 bpp, byte order B,G,R,A -------------
        // 4 bytes per pixel, width*height pixels, row padding already a
        // multiple of 4 — no stride games for this format.
        let mut color_bits = vec![0u8; (SIZE * SIZE * 4) as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let i = ((y * SIZE + x) * 4) as usize;
                if let Some((r, g, b)) = icon_pixel(x, y) {
                    color_bits[i] = b;
                    color_bits[i + 1] = g;
                    color_bits[i + 2] = r;
                    color_bits[i + 3] = 0; // alpha: ignored by this legacy path
                }
            }
        }

        // ---- AND mask: 1 bpp, bit set = transparent ---------------------
        // One bit per pixel, MSB = leftmost of each 8-pixel group; a
        // 32-px row is exactly 4 bytes (1-bpp rows round up to 4 anyway).
        let row_bytes = (SIZE as usize).div_ceil(32) * 4;
        let mut mask_bits = vec![0u8; row_bytes * SIZE as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                if icon_pixel(x, y).is_none() {
                    let byte = mask_bits[y as usize * row_bytes + (x / 8) as usize];
                    mask_bits[y as usize * row_bytes + (x / 8) as usize] = byte | (0x80 >> (x % 8)); // set = see-through
                }
            }
        }

        // nplanes=1 always; the bit count selects the depth.
        let hbm_color = CreateBitmap(SIZE, SIZE, 1, 32, Some(color_bits.as_ptr().cast()));
        let hbm_mask = CreateBitmap(SIZE, SIZE, 1, 1, Some(mask_bits.as_ptr().cast()));
        debug_assert!(!hbm_color.is_null() && !hbm_mask.is_null());

        let mut info = ICONINFO {
            fIcon: BOOL(1), // 1 = icon (0 would make a cursor; then the
            //           hotspots below would matter)
            xHotspot: 0, // cursors only
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let icon = CreateIconIndirect(&mut info);
        debug_assert!(!icon.is_null());

        // CreateIconIndirect uses the bitmaps at creation; afterwards the
        // caller still owns them — delete or they leak.
        _ = DeleteObject(hbm_color as HGDIOBJ);
        _ = DeleteObject(hbm_mask as HGDIOBJ);
        icon
    }
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            // hIcon left NULL on purpose: we set it at runtime below —
            // the class icon is only the *default*.
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e39_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e39_class"),
            w!("e39 · Runtime Icon — look at the title bar"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            420,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Generate and install the icon -----------------------------
        let icon = build_icon();
        (*state).icon = Some(icon);

        // ICON_SMALL (title bar) and ICON_BIG (Alt+Tab). Same 32×32 art
        // for both — a real app would provide 16×16 and 32/48×48.
        // WM_SETICON does NOT transfer ownership: we still destroy it in
        // WM_DESTROY.
        _ = SendMessageW(hwnd, WM_SETICON as u32, ICON_SMALL as usize, icon as isize);
        _ = SendMessageW(hwnd, WM_SETICON as u32, ICON_BIG as usize, icon as isize);

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
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // Paint the icon's exact pixels, one 8×8 block each, so
                // the user sees the same image the title bar shows —
                // driven by the very same icon_pixel() function.
                let scale = 8;
                let ox = ((rc.right - SIZE * scale) / 2).max(0); // centered
                let oy = 60;
                for y in 0..SIZE {
                    for x in 0..SIZE {
                        if let Some((r, g, b)) = icon_pixel(x, y) {
                            let brush = CreateSolidBrush(rgb(r, g, b));
                            _ = FillRect(
                                hdc,
                                &RECT {
                                    left: ox + x * scale,
                                    top: oy + y * scale,
                                    right: ox + (x + 1) * scale,
                                    bottom: oy + (y + 1) * scale,
                                },
                                brush,
                            );
                            _ = DeleteObject(brush as HGDIOBJ);
                        }
                    }
                }

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                let caption = to_wide("the title-bar icon, magnified 8× — same icon_pixel() data");
                _ = TextOutW(
                    hdc,
                    12,
                    rc.bottom - 30,
                    PCWSTR::from_raw(caption.as_ptr()),
                    -1,
                );

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                if let Some(icon) = state.icon {
                    _ = DestroyIcon(icon);
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
