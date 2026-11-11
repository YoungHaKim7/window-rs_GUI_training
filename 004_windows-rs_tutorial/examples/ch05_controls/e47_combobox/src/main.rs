//! e47 — Combobox: picking from a list, and the three-message read dance
//!
//! What you learn:
//! * The COMBOBOX control: `CBS_DROPDOWNLIST` (a locked list — compare
//!   `CBS_SIMPLE`, always expanded, and `CBS_DROPDOWN`, which also lets the
//!   user type into an edit field)
//! * **Controls are driven by messages**, not functions: you add items with
//!   `CB_ADDSTRING`, query the selection with `CB_GETCURSEL` and read the
//!   selected string with `CB_GETLBTEXTLEN` + `CB_GETLBTEXT` — all through
//!   the same `SendMessageW` you already use on windows
//! * `CBN_SELCHANGE` (the `HIWORD` of `WM_COMMAND`'s wparam) fires the
//!   moment the user picks a different entry
//!
//! New Win32 APIs: `CB_ADDSTRING`, `CB_SETCURSEL`, `CB_GETCURSEL`,
//! `CB_GETLBTEXTLEN`, `CB_GETLBTEXT`, `CreateSolidBrush`, `FillRect`.
//!
//! Run: `cargo run -p e47_combobox`

use windows::{Win32::*, core::*};

/// Control id of the combobox (WM_COMMAND's LOWORD).
const IDC_COMBO: i32 = 1001;

/// The palette on offer: display names paired with COLORREF values.
const COLORS: [(&str, COLORREF); 7] = [
    ("Red", rgb(0xC8, 0x30, 0x30)),
    ("Orange", rgb(0xE0, 0x90, 0x20)),
    ("Yellow", rgb(0xE8, 0xD0, 0x20)),
    ("Green", rgb(0x30, 0xA0, 0x40)),
    ("Cyan", rgb(0x30, 0xB0, 0xC0)),
    ("Blue", rgb(0x30, 0x50, 0xC0)),
    ("Violet", rgb(0x90, 0x40, 0xC0)),
];

#[derive(Default)]
struct State {
    selected: usize, // index into COLORS; index 0 is a fine default
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
            lpszClassName: w!("e47_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e47_class"),
            w!("e47 · Combobox — pick a color"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 360,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The combobox child ---------------------------------------------
        // CBS_DROPDOWNLIST = the classic "locked" dropdown: the user can only
        // pick, not type. The height parameter (160 here) is the height of
        // the control WITH ITS LIST FULLY DROPPED — the closed box is always
        // one line tall, no matter what you pass. That surprises everyone
        // exactly once.
        let combo = CreateWindowExW(
            0,
            w!("COMBOBOX"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST) as u32,
            20, 20, 200, 160,
            Some(hwnd), Some(IDC_COMBO as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!combo.is_null());

        // Populate the list. There is no combo_add_string() function — you
        // message the control. lparam = pointer to a null-terminated wide
        // string, so each entry needs a Vec<u16> that outlives the call.
        for (name, _) in COLORS {
            let wide = to_wide(name);
            SendMessageW(
                combo,
                CB_ADDSTRING as u32,
                0,
                wide.as_ptr() as LPARAM,
            );
        }
        // Preselect the first entry (CB_SETCURSEL's lparam is the index).
        // Doing it here means no CBN_SELCHANGE fires — we set State directly.
        SendMessageW(combo, CB_SETCURSEL as u32, 0, 0);

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

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
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

                if notification == CBN_SELCHANGE as u32 && control_id == IDC_COMBO as u32 {
                    // lparam of WM_COMMAND is the control's own HWND — handy
                    // here, since we never stored the combo in State.
                    let combo = lparam as HWND;

                    // Step 1: which index is selected? -1 = nothing selected.
                    let index =
                        SendMessageW(combo, CB_GETCURSEL as u32, 0, 0) as i32;
                    if index < 0 {
                        return 0;
                    }

                    // Step 2: how long is that entry? (Characters, sans the
                    // null terminator.) The two-call dance again — allocate
                    // exactly, then fetch.
                    let len = SendMessageW(
                        combo,
                        CB_GETLBTEXTLEN as u32,
                        index as usize,
                        0,
                    ) as i32;
                    let mut buf = vec![0u16; len as usize + 1];

                    // Step 3: copy the list-box text of that entry into the
                    // buffer. SendMessage returns the copied length.
                    let copied = SendMessageW(
                        combo,
                        CB_GETLBTEXT as u32,
                        index as usize,
                        buf.as_mut_ptr() as LPARAM,
                    ) as i32;
                    let name = String::from_utf16_lossy(&buf[..copied.max(0) as usize]);

                    state.selected = index as usize;
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!("e47 · Combobox — {name} selected")),
                    );
                    // The swatch is painted from State, so a repaint is all
                    // it takes to show the new color.
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // The swatch: a solid-brush-filled rectangle in the chosen
                // color. CreateSolidBrush takes the COLORREF we paired with
                // the selected entry — the combo only knows strings, *we*
                // know what the strings mean.
                let mut swatch = rect;
                swatch.left += 20;
                swatch.top += 70;
                swatch.right -= 20;
                swatch.bottom -= 20;
                let brush = CreateSolidBrush(COLORS[state.selected].1);
                _ = FillRect(hdc, &swatch, brush);
                _ = DeleteObject(brush as HGDIOBJ); // brushes are GDI objects

                let label = to_wide(&format!("{} — RGB 0x{:06X}",
                    COLORS[state.selected].0, COLORS[state.selected].1));
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(hdc, 20, 46,
                    PCWSTR::from_raw(label.as_ptr()), label.len() as i32 - 1);
                _ = SetBkMode(hdc, OPAQUE);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
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
