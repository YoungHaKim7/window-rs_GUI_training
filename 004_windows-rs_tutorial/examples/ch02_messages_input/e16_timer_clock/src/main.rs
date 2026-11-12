//! e16 — Timer Clock: WM_TIMER and periodic repainting
//!
//! What you learn:
//! * `SetTimer` — ask Windows to post `WM_TIMER` to your window every N
//!   milliseconds; `KillTimer` — stop it (always pair them)
//! * Timers are *low priority and coalesced*: WM_TIMER is only generated
//!   when the queue is otherwise empty, so it's a heartbeat, not a
//!   stopwatch. Never count ticks to measure time — read the clock.
//! * The standard "invalidate on tick, read fresh data in WM_PAINT" shape
//! * `GetLocalTime` + `SYSTEMTIME`
//!
//! No state struct needed here: the time lives in the system, so WM_PAINT
//! just asks for it. (State is for things only YOU know.)
//!
//! New Win32 APIs: `SetTimer`, `KillTimer`, `GetLocalTime`.
//!
//! Run: `cargo run -p e16_timer_clock`

use windows::{Win32::*, core::*};

/// Timer ids are yours to choose; they distinguish several timers on one
/// window (wparam of WM_TIMER carries the id back).
const CLOCK_TIMER: usize = 1;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e16_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e16_class"),
            w!("e16 · Timer Clock"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            260,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // Fire roughly every 250 ms. Faster than the 1 s display resolution
        // on purpose: timers drift and coalesce, so sampling 4x per second
        // keeps the seconds digit from visibly stalling or skipping.
        // The last argument is an optional callback; None = "post WM_TIMER
        // to the window instead".
        let timer = SetTimer(Some(hwnd), CLOCK_TIMER, 250, None);
        debug_assert!(timer != 0, "SetTimer failed");

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
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
            WM_TIMER => {
                // wparam = the timer id. With one timer the check is
                // optional, but it's the habit that scales.
                if wparam == CLOCK_TIMER {
                    // `false` = don't erase the background first; we paint
                    // every pixel of the text box ourselves below, which
                    // avoids the erase-then-draw flicker.
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // Read the clock NOW — never trust a tick counter.
                let mut now = SYSTEMTIME::default();
                GetLocalTime(&mut now);
                let time = format!("{:02}:{:02}:{:02}", now.wHour, now.wMinute, now.wSecond);
                let date = format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay);

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // Clear the whole client area ourselves (we invalidated
                // with erase = false).
                FillRect(hdc, &rect, GetSysColorBrush(COLOR_WINDOW));

                let big = CreateFontW(
                    96,
                    0,
                    0,
                    0,
                    FW_BOLD,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (FIXED_PITCH | FF_MODERN) as u32,
                    w!("Consolas"),
                );
                let old_font = SelectObject(hdc, big as HGDIOBJ);
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x10, 0x30, 0x70));

                let wide = to_wide(&time);
                let mut time_rect = rect;
                time_rect.bottom -= 50;
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut time_rect,
                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                );

                // Back to the default font for the date line.
                SelectObject(hdc, old_font);
                _ = DeleteObject(big as HGDIOBJ);

                _ = SetTextColor(hdc, rgb(0x60, 0x60, 0x60));
                let wide = to_wide(&date);
                let mut date_rect = rect;
                date_rect.top = rect.bottom - 50;
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut date_rect,
                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                );

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                // Timers die with their window anyway, but being explicit
                // is the habit that prevents leaks with thread timers.
                _ = KillTimer(Some(hwnd), CLOCK_TIMER);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
