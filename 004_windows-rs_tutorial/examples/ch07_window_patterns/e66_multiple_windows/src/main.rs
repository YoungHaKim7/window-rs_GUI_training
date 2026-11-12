//! e66 — Multiple Windows: two classes, two wndprocs, owner vs. parent
//!
//! What you learn:
//! * A window **class** bundles a wndproc + defaults; register two classes
//!   and you get two independent kinds of window. This is the unit of code
//!   reuse in classic Win32 — every control you know (BUTTON, EDIT, ...)
//!   is "just" a registered class with its own wndproc.
//! * **Owned vs. child windows** — the classic confusion, settled:
//!   passing a hwnd in the "parent" parameter of `CreateWindowExW` *with*
//!   `WS_CHILD` makes a child (lives inside the parent's client area,
//!   clipped, no taskbar entry). Passing the same hwnd **without**
//!   `WS_CHILD` makes an *owned* top-level window: it floats above its
//!   owner, minimizes with it, hides with it, and is destroyed when the
//!   owner is destroyed — automatically, no code from you.
//! * Per-window state via `GWLP_USERDATA` (e65 flavor: attached in
//!   WM_NCCREATE from `lpparam`), plus one honest global: an
//!   `AtomicUsize` live-note counter that the main window shows in its
//!   title. Counting windows is exactly the kind of cross-window fact
//!   where a global is the simplest correct tool.
//!
//! Click "New note" a few times; drag the notes around (they're real
//! windows); double-click a note to recolor it; close the main window and
//! watch every note vanish with it.
//!
//! New Win32 APIs: `CreateWindowExW` owner semantics, `GetTickCount`
//! (seed for jittered positions), `CreateSolidBrush`.
//!
//! Run: `cargo run -p e66_multiple_windows`

use std::sync::atomic::{AtomicUsize, Ordering};

use windows::{Win32::*, core::*};

/// Global on purpose: "how many notes exist right now" is a fact about the
/// whole app, not about any one window, so a global counter is the honest
/// shape. (Everything per-window still goes through GWLP_USERDATA.)
static LIVE_NOTES: AtomicUsize = AtomicUsize::new(0);

/// Sticky-note pastel palette. Each note owns one brush created for it.
const NOTE_COLORS: [COLORREF; 4] = [
    0x82E0_FF, // yellow  (COLORREF is 0x00BBGGRR)
    0xA8E7_B4, // green
    0xE7D4_A8, // blue
    0xC8B0_E7, // pink
];

const IDC_NEW_NOTE: i32 = 1001;

struct MainState {
    next_note_id: u32,
}

/// Per-note data. The owner hwnd rides along so a dying note can ask the
/// main window to refresh its title; the brush is this note's own GDI
/// object, destroyed with the note.
struct NoteState {
    id: u32,
    color: usize,
    brush: HBRUSH,
    owner: HWND,
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Refresh the main window's caption from the global counter. Called from
/// either wndproc — the count is shared, the title is derived, so nobody
/// needs to track anything extra.
unsafe fn update_main_title(main: HWND) {
    unsafe {
        let title = format!("e66 · Notes — {} live", LIVE_NOTES.load(Ordering::Relaxed));
        _ = SetWindowTextW(main, &HSTRING::from(title));
    }
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // Class 1: the main application window (boring defaults).
        let main_class = WNDCLASSW {
            lpfnWndProc: Some(main_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e66_main"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&main_class) != 0);

        // Class 2: sticky notes — own wndproc. The class brush is None:
        // a class has ONE brush but every note gets its own color, so
        // per-window state (WM_ERASEBKGND with the note's own brush)
        // beats per-class defaults here.
        let note_class = WNDCLASSW {
            lpfnWndProc: Some(note_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: std::ptr::null_mut(), // each note erases with its own color
            lpszClassName: w!("e66_note"),
            // CS_DBLCLKS folds two rapid clicks into WM_LBUTTONDBLCLK;
            // without it a double-click is just two WM_LBUTTONDOWNs.
            style: (CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&note_class) != 0);

        let main_state = Box::into_raw(Box::new(MainState { next_note_id: 1 }));
        let main = CreateWindowExW(
            0,
            w!("e66_main"),
            w!("e66 · Notes — 0 live"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            240,
            None,
            None,
            Some(instance),
            Some(main_state as LPVOID),
        );
        debug_assert!(!main.is_null());
        update_main_title(main);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe fn main_state_of(hwnd: HWND) -> Option<&'static mut MainState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainState;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

unsafe fn spawn_note(owner: HWND, instance: HINSTANCE, id: u32) {
    unsafe {
        let color = (id as usize) % NOTE_COLORS.len();
        // The note's state is built here and attached inside the note's
        // own WM_NCCREATE via lpparam — the e65 pattern, one box (and one
        // brush) per window, no shared mutable globals.
        let state = Box::into_raw(Box::new(NoteState {
            id,
            color,
            brush: CreateSolidBrush(NOTE_COLORS[color]),
            owner,
        }));

        // Jittered cascade position from the tick counter — not random,
        // but random-looking enough for sticky notes.
        let jitter = GetTickCount() as i32;
        let x = 140 + (jitter % 200) + (id as i32 % 4) * 40;
        let y = 140 + ((jitter / 7) % 150) + (id as i32 % 4) * 30;

        let note = CreateWindowExW(
            0,
            w!("e66_note"),
            w!("note"),
            // *** NOT WS_CHILD *** — that is the whole lesson. With
            // WS_CHILD this would be an area clipped inside the owner's
            // client rectangle. Without it, passing the main hwnd makes
            // the note an OWNED window: a free-floating top-level window
            // that nevertheless follows its owner (stays above it,
            // minimizes, hides, and is destroyed with it — automatically).
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            x,
            y,
            220,
            170,
            Some(owner), // owner (no WS_CHILD → ownership, not parenting)
            None,
            Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!note.is_null());
    }
}

extern "system" fn main_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_CREATE => {
                // State already attached — so child controls can be made
                // right here, which is exactly why the e65 flavor exists.
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;

                let button = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("New note"),
                    (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
                    20,
                    20,
                    120,
                    36,
                    Some(hwnd),
                    // The hmenu parameter doubles as the CHILD CONTROL ID
                    // for WS_CHILD windows — smuggled in as a fake HMENU.
                    Some(IDC_NEW_NOTE as *mut _), // child id
                    Some(instance),
                    None,
                );
                // The default control font is a bitmap relic; give the
                // button the standard GUI font (the ch08 mini apps do
                // this everywhere).
                _ = SendMessageW(
                    button,
                    WM_SETFONT as u32,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1, // redraw
                );
                0
            }
            WM_COMMAND => {
                // wparam low word = control id, high word = notification.
                if (wparam & 0xffff) as i32 == IDC_NEW_NOTE
                    && ((wparam >> 16) & 0xffff) as i32 == BN_CLICKED
                {
                    let Some(state) = main_state_of(hwnd) else {
                        return 0;
                    };
                    let id = state.next_note_id;
                    state.next_note_id += 1;
                    let instance = GetModuleHandleW(PCWSTR::null());
                    spawn_note(hwnd, instance, id);
                    LIVE_NOTES.fetch_add(1, Ordering::Relaxed);
                    update_main_title(hwnd);
                }
                0
            }
            WM_DESTROY => {
                // Owned windows die with their owner automatically, so we
                // never touch the notes from here; and only the main
                // window ends the message loop.
                PostQuitMessage(0);
                0
            }
            WM_NCDESTROY => {
                // Teardown mirror of WM_NCCREATE (e65): exactly one
                // from_raw, and null the slot first.
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainState;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

extern "system" fn note_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_ERASEBKGND => {
                // Per-window background color: the class couldn't know
                // it, but the state can. Returning 1 = "I erased it, skip
                // the (null anyway) class brush".
                if let Some(state) = note_state_of(hwnd) {
                    let mut rect = RECT::default();
                    _ = GetClientRect(hwnd, &mut rect);
                    _ = FillRect(wparam as HDC, &rect, state.brush);
                }
                1
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = note_state_of(hwnd) {
                    let mut rect = RECT::default();
                    _ = GetClientRect(hwnd, &mut rect);
                    rect.top += 14;
                    rect.left += 12;
                    rect.right -= 12;
                    let text = format!(
                        "Note #{}\n\nDrag me. Close the\nowner and I go too.",
                        state.id
                    );
                    let wide = to_wide(&text);
                    _ = SetBkMode(hdc, TRANSPARENT);
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut rect,
                        (DT_LEFT | DT_TOP | DT_WORDBREAK) as u32,
                    );
                }
                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_LBUTTONDBLCLK => {
                // Cycle the color: swap the brush, then repaint. The old
                // brush dies here because nothing can still have it
                // selected once FillRect returned.
                if let Some(state) = note_state_of(hwnd) {
                    state.color = (state.color + 1) % NOTE_COLORS.len();
                    let old = state.brush;
                    state.brush = CreateSolidBrush(NOTE_COLORS[state.color]);
                    _ = DeleteObject(old as HGDIOBJ);
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_NCDESTROY => {
                // The note is going away — the user closed it, OR the
                // owner is being destroyed (ownership at work: closing
                // the main window sends every note here without a single
                // line of our code). Reclaim state, then tell the main
                // window to re-count.
                let owner = note_state_of(hwnd)
                    .map(|s| s.owner)
                    .unwrap_or(std::ptr::null_mut());
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NoteState;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let boxed = Box::from_raw(ptr);
                    _ = DeleteObject(boxed.brush as HGDIOBJ);
                    drop(boxed);
                }
                LIVE_NOTES.fetch_sub(1, Ordering::Relaxed);
                if !owner.is_null() {
                    update_main_title(owner);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe fn note_state_of(hwnd: HWND) -> Option<&'static mut NoteState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NoteState;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}
