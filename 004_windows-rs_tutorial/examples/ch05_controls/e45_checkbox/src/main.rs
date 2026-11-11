//! e45 — Checkbox: a BUTTON style, not a separate class
//!
//! What you learn:
//! * There is no CHECKBOX window class. A checkbox is a BUTTON whose style
//!   is `BS_AUTOCHECKBOX` — same `w!("BUTTON")` class as e41, different `BS_`
//! * The "AUTO" in BS_AUTOCHECKBOX: the control flips its own check mark
//!   when clicked; you only *observe* via `WM_COMMAND` / `BN_CLICKED`
//! * Reading the state: `SendMessageW(hwnd_check, BM_GETCHECK, 0, 0)`
//!   returns `BST_CHECKED` / `BST_UNCHECKED`
//! * `WM_TIMER` + `SetTimer` / `KillTimer`: tick-tock without a thread
//!
//! New Win32 APIs: `BS_AUTOCHECKBOX`, `BM_GETCHECK`, `BST_CHECKED`,
//! `SetTimer`, `KillTimer`, `WM_TIMER`, `GetLocalTime`.
//!
//! Run: `cargo run -p e45_checkbox`

use windows::{Win32::*, core::*};

const IDC_SHOW_CLOCK: i32 = 1001;
const TIMER_CLOCK: usize = 1; // timer id — you pick it

#[derive(Default)]
struct State {
    hwnd_check: HWND,
    clock_on: bool,
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
            lpszClassName: w!("e45_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e45_class"),
            w!("e45 · Checkbox — tick 'Show clock'"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 460, 160,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The checkbox --------------------------------------------------
        // Same class as e41's push button: w!("BUTTON"). Only the BS_ style
        // differs — the class is a whole control family. BS_AUTOCHECKBOX
        // means "draw a box, toggle it yourself when clicked" (the non-auto
        // BS_CHECKBOX makes YOU send BM_SETCHECK — almost never what you want).
        let hwnd_check = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Show clock"),
            (WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX) as u32,
            20, 20, 200, 24,
            Some(hwnd), Some(IDC_SHOW_CLOCK as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!hwnd_check.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_check = hwnd_check;
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 && control_id == IDC_SHOW_CLOCK as u32 {
                    // The control has ALREADY flipped its box (auto style).
                    // We could trust that and keep a bool — but the honest
                    // way is to ask the control, because it owns the truth:
                    let checked =
                        SendMessageW(state.hwnd_check, BM_GETCHECK as u32, 0, 0) as i32;

                    if checked == BST_CHECKED && !state.clock_on {
                        // One timer, ticking once per second. WM_TIMER is a
                        // *synthesized* message: Windows posts it when the
                        // interval elapses — no thread involved, low
                        // resolution (~10-15 ms), perfect for a clock.
                        SetTimer(Some(hwnd), TIMER_CLOCK, 1000, None);
                        state.clock_on = true;
                    } else if checked != BST_CHECKED && state.clock_on {
                        _ = KillTimer(Some(hwnd), TIMER_CLOCK);
                        state.clock_on = false;
                        _ = SetWindowTextW(hwnd, w!("e45 · Checkbox — tick 'Show clock'"));
                    }
                }
                0
            }
            WM_TIMER => {
                // wparam's low word is the timer id we chose in SetTimer.
                if wparam == TIMER_CLOCK {
                    // GetLocalTime fills a SYSTEMTIME by value — no UTC
                    // conversion, exactly what a wall clock wants.
                    let mut now = SYSTEMTIME::default();
                    GetLocalTime(&mut now);
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!(
                            "e45 · {:02}:{:02}:{:02}",
                            now.wHour, now.wMinute, now.wSecond
                        )),
                    );
                }
                0
            }
            WM_DESTROY => {
                // Timers die with the window, but being explicit is free and
                // documents the pairing with SetTimer.
                if state.clock_on {
                    _ = KillTimer(Some(hwnd), TIMER_CLOCK);
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
