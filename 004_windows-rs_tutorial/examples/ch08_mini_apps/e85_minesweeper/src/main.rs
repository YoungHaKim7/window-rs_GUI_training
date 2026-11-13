//! e85 — Minesweeper: the whole board painted, zero buttons for cells
//!
//! What you learn:
//! * **Custom rendering as the UI**: 81 cells are drawn in WM_PAINT with
//!   GDI rectangles/text — no child controls per cell. The hit-testing
//!   is one divide (`x / CELL`): the grid IS the data structure. This is
//!   the dividing line between "controls on a form" apps and games/tools
//!   that OWN their pixels.
//! * **Flood fill** on zero-count cells: a plain worklist (Vec as stack)
//!   spreading to the 8 neighbors — the same algorithm as paint tools,
//!   here deciding which cells open for free.
//! * **First click never a mine**: mines are placed AFTER the first
//!   reveal, excluding that cell (and, kinder, its neighborhood). The
//!   board is a blank slate until you commit.
//! * Game state as an enum (Playing/Won/Lost) driving three renders:
//!   normal play, the loss reveal (all mines shown, wrong flags marked),
//!   and the win board.
//! * The restart BUTTON as the one real control — its caption is game
//!   state made visible: ":)" playing, "X(" dead, "B)" won.
//! * A one-second WM_TIMER as the clock; a flag counter derived from
//!   state ("10 - flags placed").
//!
//! Left reveals, right flags; the classic 9x9 / 10 mines rules.
//!
//! Run: `cargo run -p e85_minesweeper`

use windows::{Win32::*, core::*};

const COLS: i32 = 9;
const ROWS: i32 = 9;
const MINES: i32 = 10;
const CELL: i32 = 32;
const STRIP_H: i32 = 48; // top strip: counter | restart | clock
const TIMER_ID: usize = 1;

const IDC_RESTART: i32 = 1001;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Default)]
struct Cell {
    mine: bool,
    revealed: bool,
    flagged: bool,
    adjacent: u8, // neighboring mine count, 0..8
}

struct State {
    cells: Vec<Cell>, // COLS*ROWS, row-major
    phase: Phase,
    mines_placed: bool, // false until the first reveal
    flags: i32,
    seconds: i32,
    lcg: u32,
}

impl State {
    fn new() -> Self {
        let lcg = unsafe { GetTickCount() } | 1;
        State {
            cells: vec![Cell::default(); (COLS * ROWS) as usize],
            phase: Phase::Playing,
            mines_placed: false,
            flags: 0,
            seconds: 0,
            lcg,
        }
    }

    fn at(&self, x: i32, y: i32) -> &Cell {
        &self.cells[(y * COLS + x) as usize]
    }
    fn at_mut(&mut self, x: i32, y: i32) -> &mut Cell {
        &mut self.cells[(y * COLS + x) as usize] as _
    }

    fn rand(&mut self, n: i32) -> i32 {
        self.lcg = self.lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.lcg >> 16) as i32) % n.max(1)
    }

    /// Place MINES avoiding (fx,fy) and its 3x3 neighborhood — the
    /// "first click never dies" (and never starts stuck) guarantee.
    fn place_mines(&mut self, fx: i32, fy: i32) {
        let mut placed = 0;
        while placed < MINES {
            let (x, y) = (self.rand(COLS), self.rand(ROWS));
            let protected = (x - fx).abs() <= 1 && (y - fy).abs() <= 1;
            if protected || self.at(x, y).mine {
                continue;
            }
            self.at_mut(x, y).mine = true;
            placed += 1;
        }
        // Pre-compute adjacent counts for every cell.
        for y in 0..ROWS {
            for x in 0..COLS {
                let mut n = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (x + dx, y + dy);
                        if (dx != 0 || dy != 0)
                            && (0..COLS).contains(&nx)
                            && (0..ROWS).contains(&ny)
                            && self.at(nx, ny).mine
                        {
                            n += 1;
                        }
                    }
                }
                self.at_mut(x, y).adjacent = n as u8;
            }
        }
        self.mines_placed = true;
    }

    /// Reveal one cell; flood-fill when it counts zero neighbors.
    fn reveal(&mut self, x: i32, y: i32) {
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            if !(0..COLS).contains(&x) || !(0..ROWS).contains(&y) {
                continue;
            }
            let cell = self.at(x, y);
            if cell.revealed || cell.flagged {
                continue;
            }
            let adjacent = cell.adjacent;
            self.at_mut(x, y).revealed = true;
            if adjacent == 0 {
                // Empty cell: neighbors open for free. Push all 8; the
                // revealed/flagged checks above make it idempotent.
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx != 0 || dy != 0 {
                            stack.push((x + dx, y + dy));
                        }
                    }
                }
            }
        }
    }

    fn won(&self) -> bool {
        // Won = every mine-free cell is revealed (flags don't matter).
        self.cells.iter().all(|c| c.mine || c.revealed)
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
            hbrBackground: ((COLOR_3DFACE + 1) as isize) as HBRUSH,
            lpszClassName: w!("e85_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // Fit the frame to the board exactly (board + top strip + rim).
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: COLS * CELL + 16,
            bottom: STRIP_H + ROWS * CELL + 16,
        };
        _ = AdjustWindowRect(&mut rc, WS_OVERLAPPEDWINDOW as u32, false);

        let state = Box::into_raw(Box::new(State::new()));

        let hwnd = CreateWindowExW(
            0,
            w!("e85_class"),
            w!("e85 · Minesweeper — left reveals, right flags"),
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The classic number palette, index = adjacent count (0 unused).
const NUMBER_COLORS: [COLORREF; 9] = [
    0x00000000, // 0: empty cells draw no number
    0x00FF0000, // 1 blue
    0x00008000, // 2 green
    0x000000FF, // 3 red
    0x00800000, // 4 dark blue
    0x00000080, // 5 dark red
    0x00808000, // 6 teal
    0x00000000, // 7 black
    0x00808080, // 8 gray
];

unsafe fn face_text(phase: Phase) -> &'static str {
    match phase {
        Phase::Playing => ":)",
        Phase::Lost => "X(",
        Phase::Won => "B)",
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
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;

                // The one control: the face/restart button, centered in
                // the top strip.
                let cx = COLS * CELL + 16;
                let restart = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!(":)"),
                    (WS_CHILD | WS_VISIBLE) as u32,
                    cx / 2 - 20,
                    8,
                    40,
                    32,
                    Some(hwnd),
                    Some(IDC_RESTART as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!restart.is_null());

                // 1 Hz clock while playing.
                _ = SetTimer(Some(hwnd), TIMER_ID, 1000, None);
                0
            }

            WM_TIMER => {
                if wparam == TIMER_ID
                    && let Some(state) = state_of(hwnd)
                    && state.phase == Phase::Playing
                {
                    state.seconds += 1;
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 && id == IDC_RESTART as u32 {
                    if let Some(state) = state_of(hwnd) {
                        *state = State::new();
                    }
                    let restart = GetDlgItem(Some(hwnd), IDC_RESTART);
                    _ = SetWindowTextW(restart, w!(":)"));
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }

            WM_LBUTTONUP | WM_RBUTTONUP => {
                // Cells live below the strip; translate client -> cell.
                let x = (lparam & 0xffff) as i16 as i32;
                let y = ((lparam >> 16) & 0xffff) as i16 as i32;
                let (cx, cy) = ((x - 8) / CELL, (y - STRIP_H - 8) / CELL);
                let inside = (0..COLS).contains(&cx) && (0..ROWS).contains(&cy) && y >= STRIP_H;

                if inside {
                    let left = message as i32 == WM_LBUTTONUP;
                    // Play out the click without holding the borrow.
                    let outcome = {
                        let Some(state) = state_of(hwnd) else {
                            return 0;
                        };
                        if state.phase != Phase::Playing {
                            None
                        } else if left {
                            if state.at(cx, cy).flagged {
                                None // flags protect against misclicks
                            } else if !state.mines_placed {
                                state.place_mines(cx, cy); // first click safe
                                if state.at(cx, cy).mine {
                                    state.phase = Phase::Lost;
                                    Some(Phase::Lost)
                                } else {
                                    state.reveal(cx, cy);
                                    if state.won() {
                                        state.phase = Phase::Won;
                                        Some(Phase::Won)
                                    } else {
                                        None
                                    }
                                }
                            } else if state.at(cx, cy).mine {
                                state.phase = Phase::Lost;
                                Some(Phase::Lost)
                            } else {
                                state.reveal(cx, cy);
                                if state.won() {
                                    state.phase = Phase::Won;
                                    Some(Phase::Won)
                                } else {
                                    None
                                }
                            }
                        } else {
                            // Right click: toggle flag on closed cells.
                            if !state.at(cx, cy).revealed {
                                let now_flagged = !state.at(cx, cy).flagged;
                                state.at_mut(cx, cy).flagged = now_flagged;
                                state.flags += if now_flagged { 1 } else { -1 };
                            }
                            None
                        }
                    };

                    // Face text tracks the phase.
                    if let Some(phase) = outcome {
                        let restart = GetDlgItem(Some(hwnd), IDC_RESTART);
                        let face = to_wide(face_text(phase));
                        _ = SetWindowTextW(restart, PCWSTR::from_raw(face.as_ptr()));
                    }
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // ---- Top strip: counter, clock ----------------------
                _ = SetBkMode(hdc, TRANSPARENT);
                if let Some(state) = state_of(hwnd) {
                    _ = SetTextColor(hdc, rgb(0xC0, 0x00, 0x00));
                    let counter = to_wide(&format!("{:03}", (MINES - state.flags).max(-99)));
                    _ = TextOutW(hdc, 12, 16, PCWSTR::from_raw(counter.as_ptr()), -1);
                    let clock = to_wide(&format!("{:03}", state.seconds.min(999)));
                    _ = TextOutW(
                        hdc,
                        COLS * CELL + 16 - 56,
                        16,
                        PCWSTR::from_raw(clock.as_ptr()),
                        -1,
                    );
                }

                // ---- The board ---------------------------------------
                let closed = CreateSolidBrush(rgb(0xC0, 0xC0, 0xC0)); // raised cell
                let opened = CreateSolidBrush(rgb(0xE0, 0xE0, 0xE0)); // flat cell
                let mine_brush = CreateSolidBrush(rgb(0x10, 0x10, 0x10));
                let flag_brush = CreateSolidBrush(rgb(0xC0, 0x20, 0x20));

                for gy in 0..ROWS {
                    for gx in 0..COLS {
                        let x = 8 + gx * CELL;
                        let y = STRIP_H + 8 + gy * CELL;
                        let cell_rect = RECT {
                            left: x,
                            top: y,
                            right: x + CELL,
                            bottom: y + CELL,
                        };

                        let cell = state_of(hwnd).map(|s| *s.at(gx, gy)).unwrap_or_default();
                        let phase = state_of(hwnd).map(|s| s.phase).unwrap_or(Phase::Playing);

                        // Base fill: closed cells get the raised gray, open
                        // ones the flat lighter tone.
                        let show_content = cell.revealed || (phase != Phase::Playing && cell.mine);
                        _ = FillRect(hdc, &cell_rect, if show_content { opened } else { closed });

                        if show_content {
                            if cell.mine {
                                // A mine: dot + spikes in every direction.
                                let cx = x + CELL / 2;
                                let cy = y + CELL / 2;
                                if cell.revealed || phase == Phase::Lost {
                                    let old = SelectObject(hdc, mine_brush as HGDIOBJ);
                                    _ = Ellipse(hdc, cx - 6, cy - 6, cx + 6, cy + 6);
                                    SelectObject(hdc, old);
                                    let pen = CreatePen(PS_SOLID, 2, rgb(0x10, 0x10, 0x10));
                                    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                                        _ = MoveToEx(hdc, cx - dx * 10, cy - dy * 10, None);
                                        _ = LineTo(hdc, cx + dx * 10, cy + dy * 10);
                                    }
                                    SelectObject(hdc, old_pen);
                                    _ = DeleteObject(pen as HGDIOBJ);
                                }
                                if cell.flagged {
                                    // Correctly flagged mine: keep the flag.
                                    let old = SelectObject(hdc, flag_brush as HGDIOBJ);
                                    _ = Rectangle(hdc, cx - 4, cy - 8, cx + 4, cy + 8);
                                    SelectObject(hdc, old);
                                }
                            } else if cell.adjacent > 0 {
                                // Neighbor count, in the classic palette.
                                _ = SetTextColor(hdc, NUMBER_COLORS[cell.adjacent as usize]);
                                let wide = to_wide(&cell.adjacent.to_string());
                                let mut r = cell_rect;
                                _ = DrawTextW(
                                    hdc,
                                    PCWSTR::from_raw(wide.as_ptr()),
                                    -1,
                                    &mut r,
                                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                                );
                            }
                        } else if cell.flagged {
                            // Closed + flagged: a little pennant.
                            let cx = x + CELL / 2;
                            let cy = y + CELL / 2;
                            let old = SelectObject(hdc, flag_brush as HGDIOBJ);
                            _ = Rectangle(hdc, cx - 4, cy - 8, cx + 4, cy + 8);
                            SelectObject(hdc, old);
                            // A wrong flag after loss gets an X through it.
                            if phase == Phase::Lost {
                                let pen = CreatePen(PS_SOLID, 2, rgb(0xC0, 0x00, 0x00));
                                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                                _ = MoveToEx(hdc, x + 4, y + 4, None);
                                _ = LineTo(hdc, x + CELL - 4, y + CELL - 4);
                                _ = MoveToEx(hdc, x + CELL - 4, y + 4, None);
                                _ = LineTo(hdc, x + 4, y + CELL - 4);
                                SelectObject(hdc, old_pen);
                                _ = DeleteObject(pen as HGDIOBJ);
                            }
                        }

                        // Cell grid lines.
                        let pen = CreatePen(PS_SOLID, 1, rgb(0x80, 0x80, 0x80));
                        let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                        let br = SelectObject(hdc, GetStockObject(NULL_BRUSH));
                        _ = Rectangle(
                            hdc,
                            cell_rect.left,
                            cell_rect.top,
                            cell_rect.right,
                            cell_rect.bottom,
                        );
                        SelectObject(hdc, br);
                        SelectObject(hdc, old_pen);
                        _ = DeleteObject(pen as HGDIOBJ);
                    }
                }

                _ = DeleteObject(closed as HGDIOBJ);
                _ = DeleteObject(opened as HGDIOBJ);
                _ = DeleteObject(mine_brush as HGDIOBJ);
                _ = DeleteObject(flag_brush as HGDIOBJ);

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
