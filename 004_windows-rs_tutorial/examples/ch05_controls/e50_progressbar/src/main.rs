//! e50 — Progress Bars: common controls, and why InitCommonControlsEx first
//!
//! What you learn:
//! * Everything from `msctls_*` classes onward is a **common control**
//!   (comctl32.dll), not a USER32 built-in: you must register the family
//!   with `InitCommonControlsEx` — with an `ICC_*` flag naming which
//!   family — before creating one, or the class doesn't exist yet
//! * The progress bar (`msctls_progress32`): range via `PBM_SETRANGE32`,
//!   increment via `PBM_SETSTEP` + `PBM_STEPIT` — driven here by a
//!   `WM_TIMER` from `SetTimer`
//! * `PBS_MARQUEE` + `PBM_SETMARQUEE` — the animated "something is
//!   happening" bar for work whose duration you CANNOT know (connecting,
//!   searching); use the determinate bar when you CAN report progress
//!
//! New Win32 APIs: `InitCommonControlsEx`, `INITCOMMONCONTROLSEX`,
//! `PBM_SETRANGE32`, `PBM_SETSTEP`, `PBM_STEPIT`, `PBM_GETPOS`,
//! `PBS_MARQUEE`, `PBM_SETMARQUEE`, `SetTimer`, `KillTimer`.
//!
//! Run: `cargo run -p e50_progressbar`

use windows::{Win32::*, core::*};

/// Timer id — WM_TIMER's wparam tells you which timer fired.
const TIMER_ID: usize = 1;
const TIMER_MS: u32 = 100; // one step every 100 ms → full sweep in 10 s

#[derive(Default)]
struct State {
    hwnd_determinate: HWND, // the classic 0..100 bar
    hwnd_marquee: HWND, // the forever-animating bar
}

fn main() {
    unsafe {
        // ---- Register the common-control families we need ---------------
        // BUTTON/EDIT/STATIC/... live in user32 and always exist. The
        // comctl32 controls (progress bar, trackbar, updown, status bar,
        // toolbar...) are registered per-family on demand — historically by
        // InitCommonControls(), today by this Ex version with explicit
        // ICC_* flags (ICC_PROGRESS_CLASS here; e51/e53/e54 use their own).
        // Calling it before CreateWindowExW is the whole point: the class
        // "msctls_progress32" literally does not exist until comctl32 has
        // been asked to provide it.
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e50_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e50_class"),
            w!("e50 · Progress Bars — determinate and marquee"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 480, 240,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Bar 1: determinate (we know the fraction) --------------------
        // PROGRESS_CLASS is the exported name for "msctls_progress32"; the
        // literal works the same way and matches how we name the other
        // comctl32 classes in this chapter.
        let bar1 = CreateWindowExW(
            0,
            w!("msctls_progress32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20, 40, 420, 24,
            Some(hwnd), None, Some(instance), None,
        );
        debug_assert!(!bar1.is_null());

        // Configure by message, as with every control:
        SendMessageW(bar1, PBM_SETRANGE32 as u32, 0, 100); // min in wparam, max in lparam
        SendMessageW(bar1, PBM_SETSTEP as u32, 0, 5); // each PBM_STEPIT advances 5
        // (PBM_SETSTEP's step rides in lparam; wparam is unused.)

        // ---- Bar 2: marquee (we don't know the fraction) -------------------
        // PBS_MARQUEE in the style is what switches the bar into animation
        // mode — but the animation only STARTS when you also send
        // PBM_SETMARQUEE(TRUE, elapsed_ms). The 30 ms here is the per-frame
        // time: smaller = smoother/faster crawl.
        let bar2 = CreateWindowExW(
            0,
            w!("msctls_progress32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | PBS_MARQUEE) as u32,
            20, 120, 420, 24,
            Some(hwnd), None, Some(instance), None,
        );
        debug_assert!(!bar2.is_null());
        SendMessageW(bar2, PBM_SETMARQUEE as u32, 1, 30); // TRUE, 30 ms

        if let Some(s) = state_of(hwnd) {
            s.hwnd_determinate = bar1;
            s.hwnd_marquee = bar2;
        }

        // The pump of the animation: WM_TIMER every TIMER_MS.
        SetTimer(Some(hwnd), TIMER_ID, TIMER_MS, None);

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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_TIMER => {
                if wparam == TIMER_ID {
                    // Advance one step. At the top of the range the bar
                    // wraps back to the minimum on its own — PBM_STEPIT is
                    // modulo its range, which suits a demo nicely.
                    SendMessageW(state.hwnd_determinate, PBM_STEPIT as u32, 0, 0);

                    // Ask the bar where it is (don't track it yourself) and
                    // mirror it into the caption. Single source of truth:
                    // the control.
                    let pos = SendMessageW(
                        state.hwnd_determinate,
                        PBM_GETPOS as u32,
                        0,
                        0,
                    ) as i32;
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!("e50 · Progress Bars — {pos}%")),
                    );
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // Static labels so the two bars need no explanation.
                let l1 = to_wide("Determinate: 0..100 in PBM_SETSTEP=5 steps \
                                  (WM_TIMER + PBM_STEPIT)");
                let l2 = to_wide("Marquee: use when the duration is unknown \
                                  (PBS_MARQUEE + PBM_SETMARQUEE)");
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(hdc, 20, 16, PCWSTR::from_raw(l1.as_ptr()), l1.len() as i32 - 1);
                _ = TextOutW(hdc, 20, 96, PCWSTR::from_raw(l2.as_ptr()), l2.len() as i32 - 1);
                _ = SetBkMode(hdc, OPAQUE);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                _ = KillTimer(Some(hwnd), TIMER_ID); // tidy: timers are resources
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
