//! e84 — Snake: a game loop made of WM_TIMER, double-buffered
//!
//! What you learn:
//! * The **timer-driven game loop**: no threads, no "game engine" — the
//!   heartbeat is WM_TIMER; each tick advances the snake one cell.
//!   Speeding up = `KillTimer` + `SetTimer` with a smaller interval
//!   (timers can't be re-armed in place), floored so the game stays
//!   playable.
//! * The model is plain Rust (`Vec<(i32,i32)>` snake, food cell,
//!   direction); WM_PAINT is a pure function of it. Input (arrow keys)
//!   writes a *pending* direction consumed on the next tick — instant
//!   reversal is rejected, the classic rule that stops you from eating
//!   your own neck by pressing two keys within one tick.
//! * Double buffering, the e26 recipe, at game speed: draw the frame
//!   into a memory DC, `BitBlt` once. Without it the snake visibly
//!   strobes on every tick.
//! * Randomness with zero dependencies: a one-line LCG
//!   (`seed = seed*1664525 + 1013904223`) seeded from `GetTickCount` —
//!   good enough for food placement, and honest about not being crypto.
//! * Game over via MessageBox with **retry** — and the re-entrancy rule
//!   from e65 applied for real: no `&mut State` alive across the box.
//!
//! Arrows steer; walls and your own body kill; each food = +1 score and
//! a faster timer. Watch the title bar.
//!
//! New Win32 APIs: `KillTimer` (re-arm for speed), `GetTickCount`
//! (sysinfoapi — the LCG seed), everything else is recap.
//!
//! Run: `cargo run -p e84_snake`

use windows::{Win32::*, core::*};

const COLS: i32 = 32;
const ROWS: i32 = 22;
const CELL: i32 = 20;
const TIMER_ID: usize = 1;
const START_MS: u32 = 150;
const FASTEST_MS: u32 = 60;

struct State {
    snake: Vec<(i32, i32)>, // head first
    dir: (i32, i32),
    pending_dir: (i32, i32),
    food: (i32, i32),
    score: i32,
    alive: bool,
    interval_ms: u32,
    lcg: u32, // tiny PRNG state (see next_food)
}

impl State {
    fn new() -> Self {
        // The seed only needs to differ between runs — GetTickCount is
        // exactly that. (Honest: an LCG is not a real PRNG, but food
        // placement doesn't need one.)
        let lcg = unsafe { GetTickCount() } | 1;
        let mut state = State {
            snake: vec![(8, ROWS / 2), (7, ROWS / 2), (6, ROWS / 2)],
            dir: (1, 0),
            pending_dir: (1, 0),
            food: (0, 0),
            score: 0,
            alive: true,
            interval_ms: START_MS,
            lcg,
        };
        state.next_food();
        state
    }

    fn rand(&mut self, n: i32) -> i32 {
        // Classic numerical-recipes LCG; take high bits (low bits of an
        // LCG have poor low-order entropy).
        self.lcg = self.lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.lcg >> 16) as i32) % n.max(1)
    }

    fn next_food(&mut self) {
        loop {
            let f = (self.rand(COLS), self.rand(ROWS));
            if !self.snake.contains(&f) {
                self.food = f;
                return;
            }
        }
    }
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
            lpszClassName: w!("e84_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // Client area must exactly fit COLS*ROWS cells: ask for the
        // frame that yields it.
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: COLS * CELL,
            bottom: ROWS * CELL,
        };
        _ = AdjustWindowRect(&mut rc, WS_OVERLAPPEDWINDOW as u32, false);

        let state = Box::into_raw(Box::new(State::new()));

        let hwnd = CreateWindowExW(
            0,
            w!("e84_class"),
            w!("e84 · Snake — arrows steer"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rc.right - rc.left,
            rc.bottom - rc.top,
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

unsafe fn update_title(hwnd: HWND) {
    unsafe {
        let (score, alive, ms) = match state_of(hwnd) {
            Some(s) => (s.score, s.alive, s.interval_ms),
            None => (0, true, START_MS),
        };
        let status = if alive {
            format!("score {score} · {ms} ms/tick")
        } else {
            "DEAD".into()
        };
        _ = SetWindowTextW(hwnd, &HSTRING::from(format!("e84 · Snake — {status}")));
    }
}

/// Advance one cell. Returns true when the game just ended.
fn step(state: &mut State) -> bool {
    // Consume the queued direction — but a direct U-turn is refused
    // (the head can't enter the neck's cell).
    if state.pending_dir != (-state.dir.0, -state.dir.1) {
        state.dir = state.pending_dir;
    }
    let (hx, hy) = state.snake[0];
    let head = (hx + state.dir.0, hy + state.dir.1);

    // Walls kill.
    if head.0 < 0 || head.0 >= COLS || head.1 < 0 || head.1 >= ROWS {
        state.alive = false;
        return true;
    }
    // Body kills. NOTE: the tail cell vacates this same tick UNLESS we
    // just ate (then nothing moves out) — the strict version checks
    // eating first; the forgiving version below lets you chase the tail.
    // We keep it simple and forgiving-tail-free: any body hit kills.
    if state.snake.contains(&head) {
        state.alive = false;
        return true;
    }

    state.snake.insert(0, head);
    if head == state.food {
        // Ate: keep the tail (grow), respawn food.
        state.score += 1;
        state.next_food();
        false
    } else {
        state.snake.pop();
        false
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
                _ = SetTimer(Some(hwnd), TIMER_ID, START_MS, None);
                update_title(hwnd);
                0
            }

            WM_TIMER => {
                if wparam != TIMER_ID {
                    return 0;
                }
                // Compute the step WITHOUT holding the borrow across the
                // possible MessageBox below (e65: it re-enters wndproc).
                let (died, ate) = {
                    let Some(state) = state_of(hwnd) else {
                        return 0;
                    };
                    if !state.alive {
                        return 0;
                    }
                    let died = step(state);
                    (died, state.snake[0] == state.food)
                };
                let _ = ate;

                if died {
                    // No &mut alive here — safe to run the box's loop.
                    let retry = MessageBoxW(
                        Some(hwnd),
                        &HSTRING::from(format!(
                            "You died with a score of {}.\n\nAnother run?",
                            score_of(hwnd)
                        )),
                        w!("e84 · Snake"),
                        (MB_RETRYCANCEL | MB_ICONQUESTION) as u32,
                    ) == IDRETRY;
                    if retry {
                        if let Some(state) = state_of(hwnd) {
                            *state = State::new();
                        }
                        // Fresh game = fresh clock.
                        rearm_timer(hwnd, START_MS);
                    }
                    update_title(hwnd);
                    _ = InvalidateRect(Some(hwnd), None, false);
                    return 0;
                }

                // Speed up as you eat: re-arm the timer a hair faster.
                // (Feedback loop trimmed: interval = f(score).)
                let interval = match state_of(hwnd) {
                    Some(state) => (START_MS - (state.score as u32) * 6).max(FASTEST_MS),
                    None => START_MS,
                };
                if let Some(state) = state_of(hwnd)
                    && interval != state.interval_ms
                {
                    state.interval_ms = interval;
                    rearm_timer(hwnd, interval);
                }
                update_title(hwnd);
                _ = InvalidateRect(Some(hwnd), None, false);
                0
            }

            WM_KEYDOWN => {
                let dir = match wparam as i32 {
                    VK_LEFT => Some((-1, 0)),
                    VK_RIGHT => Some((1, 0)),
                    VK_UP => Some((0, -1)),
                    VK_DOWN => Some((0, 1)),
                    _ => None,
                };
                if let Some(dir) = dir
                    && let Some(state) = state_of(hwnd)
                {
                    state.pending_dir = dir;
                }
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);

                if w > 0 && h > 0 {
                    // ---- e26 double buffer: draw offscreen, blit once --
                    let memdc = CreateCompatibleDC(Some(hdc));
                    let bitmap = CreateCompatibleBitmap(hdc, w, h); // compatible with the SCREEN dc!
                    let old_bitmap = SelectObject(memdc, bitmap as HGDIOBJ);

                    // Field background.
                    let field = CreateSolidBrush(rgb(0x1E, 0x2A, 0x1E));
                    _ = FillRect(memdc, &rc, field);
                    _ = DeleteObject(field as HGDIOBJ);

                    // Faint checker to make cells readable.
                    let alt = CreateSolidBrush(rgb(0x24, 0x33, 0x24));
                    for gy in 0..ROWS {
                        for gx in 0..COLS {
                            if (gx + gy) % 2 == 0 {
                                _ = FillRect(
                                    memdc,
                                    &RECT {
                                        left: gx * CELL,
                                        top: gy * CELL,
                                        right: (gx + 1) * CELL,
                                        bottom: (gy + 1) * CELL,
                                    },
                                    alt,
                                );
                            }
                        }
                    }
                    _ = DeleteObject(alt as HGDIOBJ);

                    if let Some(state) = state_of(hwnd) {
                        // Food: a red dot.
                        let food_brush = CreateSolidBrush(rgb(0xD0, 0x40, 0x40));
                        let old_brush = SelectObject(memdc, food_brush as HGDIOBJ);
                        _ = Ellipse(
                            memdc,
                            state.food.0 * CELL + 3,
                            state.food.1 * CELL + 3,
                            (state.food.0 + 1) * CELL - 3,
                            (state.food.1 + 1) * CELL - 3,
                        );
                        SelectObject(memdc, old_brush);
                        _ = DeleteObject(food_brush as HGDIOBJ);

                        // Snake: body segments, head brighter.
                        for (i, &(x, y)) in state.snake.iter().enumerate() {
                            let shade = if i == 0 { 0x70 } else { 0x50 };
                            let seg =
                                CreateSolidBrush(rgb(shade, (shade as f32 * 1.6) as u8, shade / 2));
                            _ = FillRect(
                                memdc,
                                &RECT {
                                    left: x * CELL + 1,
                                    top: y * CELL + 1,
                                    right: (x + 1) * CELL - 1,
                                    bottom: (y + 1) * CELL - 1,
                                },
                                seg,
                            );
                            _ = DeleteObject(seg as HGDIOBJ);
                        }
                    }

                    _ = BitBlt(hdc, 0, 0, w, h, Some(memdc), 0, 0, SRCCOPY);

                    SelectObject(memdc, old_bitmap);
                    _ = DeleteObject(bitmap as HGDIOBJ);
                    _ = DeleteDC(memdc);
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
                _ = KillTimer(Some(hwnd), TIMER_ID);
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe fn rearm_timer(hwnd: HWND, ms: u32) {
    unsafe {
        // Timers cannot change interval in place — kill, then set.
        _ = KillTimer(Some(hwnd), TIMER_ID);
        _ = SetTimer(Some(hwnd), TIMER_ID, ms, None);
    }
}

unsafe fn score_of(hwnd: HWND) -> i32 {
    unsafe { state_of(hwnd).map(|s| s.score).unwrap_or(0) }
}
