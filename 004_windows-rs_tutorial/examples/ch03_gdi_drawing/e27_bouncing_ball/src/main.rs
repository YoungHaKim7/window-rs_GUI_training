//! e27 — Bouncing Ball: timers + double buffering = animation
//!
//! What you learn:
//! * The anatomy of a GDI animation loop: `SetTimer` (~15 ms) →
//!   WM_TIMER advances the physics → `InvalidateRect` → WM_PAINT renders
//!   the *entire frame from state* (nothing incremental, nothing XOR)
//! * e26's double buffering doing real work: without it a moving ball
//!   tears and flickers at 60+ fps
//! * The State struct from e13 carrying simulation data (position,
//!   velocity) instead of document data
//! * WM_SIZE keeping the simulation inside the window (clamp on shrink)
//! * The honest simplification: physics advances by a FIXED step per
//!   WM_TIMER tick. WM_TIMER is low-priority and coalesces under load
//!   (e16), so the ball may briefly run "slow" on a busy machine. Real
//!   games measure elapsed time (QueryPerformanceCounter) and integrate
//!   dt — that's a chapter-9 topic, not today's.
//!
//! New Win32 APIs: none beyond e26's — that's the point: animation is a
//! composition of patterns you already own.
//!
//! Run: `cargo run -p e27_bouncing_ball`

use windows::{Win32::*, core::*};

const TICK_MS: u32 = 15; // ~66 fps requested; the timer coalesces below that
const BALL_RADIUS: i32 = 18;
const TIMER_ID: usize = 1;

#[derive(Default)]
struct State {
    pos: (f32, f32), // ball center, client pixels
    vel: (f32, f32), // pixels per tick
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
            lpszClassName: w!("e27_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e27_class"),
            w!("e27 · Bouncing Ball"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            420,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // Initial conditions — pick via the pointer, so WM_SIZE arriving
        // later can clamp them. A diagonal launch, classic DVD-player.
        let state = Box::into_raw(Box::new(State {
            pos: (120.0, 90.0),
            vel: (5.2, 3.4),
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        _ = SetTimer(Some(hwnd), TIMER_ID, TICK_MS, None);

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

/// Advance the simulation one fixed step and bounce off the walls.
fn step(state: &mut State, client: (i32, i32)) {
    let r = BALL_RADIUS as f32;
    state.pos.0 += state.vel.0;
    state.pos.1 += state.vel.1;

    // Reflect at the walls; clamp so the ball never escapes on a resize.
    if state.pos.0 < r {
        state.pos.0 = r;
        state.vel.0 = state.vel.0.abs();
    }
    if state.pos.0 > client.0 as f32 - r {
        state.pos.0 = client.0 as f32 - r;
        state.vel.0 = -state.vel.0.abs();
    }
    if state.pos.1 < r {
        state.pos.1 = r;
        state.vel.1 = state.vel.1.abs();
    }
    if state.pos.1 > client.1 as f32 - r {
        state.pos.1 = client.1 as f32 - r;
        state.vel.1 = -state.vel.1.abs();
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_TIMER => {
                if wparam == TIMER_ID {
                    let mut rc = RECT::default();
                    _ = GetClientRect(hwnd, &mut rc);
                    step(state, (rc.right, rc.bottom));

                    // erase = false: WM_PAINT repaints every pixel itself
                    // (into the offscreen buffer), so pre-erasing would
                    // only add a pointless fill + flash risk.
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }
            WM_SIZE => {
                // The window shrank under the ball? Pull it back inside;
                // reflection in step() handles direction. (Minimized → 0×0:
                // step() clamps to radius, harmless.)
                let w = (lparam & 0xffff) as i16 as i32;
                let h = ((lparam >> 16) & 0xffff) as i16 as i32;
                let r = BALL_RADIUS as f32;
                state.pos.0 = state.pos.0.clamp(r, (w as f32 - r).max(r));
                state.pos.1 = state.pos.1.clamp(r, (h as f32 - r).max(r));
                0
            }
            WM_ERASEBKGND => 1, // e26: we paint the background ourselves
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);

                if w > 0 && h > 0 {
                    // e26's recipe verbatim: offscreen DC sized to the
                    // client, draw, blit, cleanup.
                    let memdc = CreateCompatibleDC(Some(hdc));
                    let bitmap = CreateCompatibleBitmap(hdc, w, h);
                    let old_bitmap = SelectObject(memdc, bitmap as HGDIOBJ);

                    // Backdrop: dark court with a center line.
                    let bg = CreateSolidBrush(rgb(0x18, 0x22, 0x33));
                    _ = FillRect(memdc, &rc, bg);
                    _ = DeleteObject(bg as HGDIOBJ);
                    let pen = CreatePen(PS_SOLID, 2, rgb(0x40, 0x60, 0x80));
                    let old_pen = SelectObject(memdc, pen as HGDIOBJ);
                    _ = MoveToEx(memdc, 0, h / 2, None);
                    _ = LineTo(memdc, w, h / 2);
                    SelectObject(memdc, old_pen);
                    _ = DeleteObject(pen as HGDIOBJ);

                    // The ball: body + a small highlight offset up-left,
                    // plus a shadow ellipse beneath for depth.
                    let (x, y) = (state.pos.0 as i32, state.pos.1 as i32);
                    let shadow = CreateSolidBrush(rgb(0x10, 0x16, 0x22));
                    _ = SelectObject(memdc, shadow as HGDIOBJ);
                    _ = Ellipse(
                        memdc,
                        x - BALL_RADIUS + 4,
                        y - BALL_RADIUS + 6,
                        x + BALL_RADIUS + 4,
                        y + BALL_RADIUS + 6,
                    );
                    let body = CreateSolidBrush(rgb(0xE0, 0x50, 0x30));
                    _ = SelectObject(memdc, body as HGDIOBJ);
                    _ = Ellipse(
                        memdc,
                        x - BALL_RADIUS,
                        y - BALL_RADIUS,
                        x + BALL_RADIUS,
                        y + BALL_RADIUS,
                    );
                    let shine = CreateSolidBrush(rgb(0xFF, 0xC0, 0xA0));
                    _ = SelectObject(memdc, shine as HGDIOBJ);
                    _ = Ellipse(memdc, x - 9, y - 11, x - 1, y - 3);
                    // Deselect the last brush before deleting it.
                    _ = SelectObject(memdc, GetStockObject(BLACK_BRUSH));
                    _ = DeleteObject(shadow as HGDIOBJ);
                    _ = DeleteObject(body as HGDIOBJ);
                    _ = DeleteObject(shine as HGDIOBJ);

                    _ = SetBkMode(memdc, TRANSPARENT);
                    _ = SetTextColor(memdc, rgb(0x88, 0x99, 0xAA));
                    _ = TextOutW(
                        memdc,
                        10,
                        8,
                        w!("fixed-step physics + double buffering"),
                        -1,
                    );

                    _ = BitBlt(hdc, 0, 0, w, h, Some(memdc), 0, 0, SRCCOPY);
                    SelectObject(memdc, old_bitmap);
                    _ = DeleteObject(bitmap as HGDIOBJ);
                    _ = DeleteDC(memdc);
                }

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                _ = KillTimer(Some(hwnd), TIMER_ID);
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
