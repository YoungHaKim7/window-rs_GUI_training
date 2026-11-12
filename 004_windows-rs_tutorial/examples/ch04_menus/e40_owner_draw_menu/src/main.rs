//! e40 — Owner-Draw Menus: you paint the items yourself
//!
//! What you learn:
//! * `MF_OWNERDRAW` — flag an item as "I will render this". Windows stops
//!   drawing it entirely; instead it asks you twice per menu: how big is
//!   it (`WM_MEASUREITEM`), then paint it (`WM_DRAWITEM`) — every time it
//!   is shown, hovered, or highlighted
//! * `WM_MEASUREITEM` — lparam points at a `MEASUREITEMSTRUCT`; you fill
//!   `itemWidth`/`itemHeight` and return TRUE. `CtlType == ODT_MENU`
//!   says the question is about a menu (the same message serves owner-
//!   draw controls like listboxes later in the tutorial)
//! * `WM_DRAWITEM` — lparam points at a `DRAWITEMSTRUCT` with everything
//!   you need: which item (`itemID`), where (`rcItem`), with what
//!   (`hDC` — a borrowed DC, no BeginPaint), and in what state
//!   (`itemState`: `ODS_SELECTED` = the mouse is on it, `ODS_GRAYED`, ...)
//! * The render here: a color swatch + label per item, highlight row
//!   behind the selected one — the classic palette menu
//!
//! Open Palette, watch items repaint as you hover, and pick a color.
//!
//! New Win32 APIs: `MF_OWNERDRAW`, `WM_MEASUREITEM`, `WM_DRAWITEM`,
//! `MEASUREITEMSTRUCT`, `DRAWITEMSTRUCT`, `ODT_MENU`/`ODS_SELECTED`
//! (winuser).
//!
//! Run: `cargo run -p e40_owner_draw_menu`

use windows::{Win32::*, core::*};

/// The palette: (menu id, label, red, green, blue). itemID indexes this.
const PALETTE: [(usize, &str, u8, u8, u8); 5] = [
    (6001, "Crimson", 0xDC, 0x14, 0x3C),
    (6002, "Gold", 0xE8, 0xB4, 0x2A),
    (6003, "Sea green", 0x2E, 0x8B, 0x57),
    (6004, "Slate blue", 0x4B, 0x6A, 0xB8),
    (6005, "Orchid", 0xA3, 0x5A, 0xC7),
];

#[derive(Default)]
struct State {
    picked: Option<(COLORREF, &'static str)>,
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
            lpszClassName: w!("e40_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e40_class"),
            w!("e40 · Owner-Draw Menu — Palette"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            340,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The owner-draw palette menu --------------------------------
        let bar = CreateMenu();
        let palette_menu = CreatePopupMenu();
        for (id, label, _, _, _) in PALETTE {
            // MF_OWNERDRAW: no text will be drawn by the system. The
            // string slot would become "itemData" (a ULONG_PTR handed
            // back in the structs) — but itemData must travel as a
            // PCWSTR-shaped parameter here, so we pass an empty string
            // and look the palette up by itemID instead. Same trick the
            // C examples play with pointers, minus the unsafe plumbing.
            _ = AppendMenuW(palette_menu, MF_OWNERDRAW as u32, id, w!(""));
            let _ = label; // label is used in the draw callback below
        }
        _ = AppendMenuW(bar, MF_POPUP as u32, palette_menu as usize, w!("&Palette"));
        _ = SetMenu(hwnd, Some(bar));

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        _ = DestroyMenu(bar);
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
            WM_MEASUREITEM => {
                // "How big is item N?" — asked once, just before the
                // menu opens. lparam → MEASUREITEMSTRUCT.
                let mis = &mut *(lparam as *mut MEASUREITEMSTRUCT);

                if mis.CtlType == ODT_MENU as u32 {
                    let Some((_, label, _, _, _)) = PALETTE
                        .iter()
                        .find(|(id, _, _, _, _)| *id as u32 == mis.itemID)
                    else {
                        return 0;
                    };
                    // Width: swatch + gap + text (≈8 px per character is a
                    // decent Segoe UI estimate at 16 px height — measuring
                    // with GetTextExtentPoint32W is the exact way).
                    mis.itemWidth = 46 + label.len() as u32 * 8 + 16;
                    mis.itemHeight = 26;
                    1 // TRUE: "we handled it"
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_DRAWITEM => {
                // "Paint item N, now." — asked every time the item's
                // appearance changes: shown, hovered, un-hovered...
                let dis = &mut *(lparam as *mut DRAWITEMSTRUCT);

                if dis.CtlType == ODT_MENU as u32 {
                    let Some((_, label, r, g, b)) = PALETTE
                        .iter()
                        .find(|(id, _, _, _, _)| *id as u32 == dis.itemID)
                    else {
                        return 0;
                    };

                    // Hover state arrives in itemState — repaint the
                    // background so the user sees which row is hot.
                    let selected = dis.itemState & ODS_SELECTED as u32 != 0;
                    let bg = CreateSolidBrush(if selected {
                        rgb(0xD8, 0xE4, 0xF8) // highlight row
                    } else {
                        rgb(0xF6, 0xF6, 0xF6) // normal row
                    });
                    _ = FillRect(dis.hDC, &dis.rcItem, bg);
                    _ = DeleteObject(bg as HGDIOBJ);

                    // The color swatch: a little inset colored rect.
                    let swatch = CreateSolidBrush(rgb(*r, *g, *b));
                    _ = FillRect(
                        dis.hDC,
                        &RECT {
                            left: dis.rcItem.left + 6,
                            top: dis.rcItem.top + 5,
                            right: dis.rcItem.left + 38,
                            bottom: dis.rcItem.bottom - 5,
                        },
                        swatch,
                    );
                    _ = DeleteObject(swatch as HGDIOBJ);

                    // The label, vertically centered by hand.
                    _ = SetBkMode(dis.hDC, TRANSPARENT);
                    _ = SetTextColor(dis.hDC, rgb(0x20, 0x20, 0x20));
                    let wide = to_wide(label);
                    _ = TextOutW(
                        dis.hDC,
                        dis.rcItem.left + 46,
                        dis.rcItem.top + 5,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                    );
                    1 // TRUE: handled
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                if let Some((_, label, r, g, b)) =
                    PALETTE.iter().find(|(pid, _, _, _, _)| *pid == id)
                {
                    state.picked = Some((rgb(*r, *g, *b), label));
                    _ = InvalidateRect(Some(hwnd), None, true);
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                if let Some((color, name)) = state.picked {
                    let brush = CreateSolidBrush(color);
                    _ = FillRect(hdc, &rc, brush);
                    _ = DeleteObject(brush as HGDIOBJ);
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF));
                    let line = to_wide(name);
                    _ = TextOutW(hdc, 14, 14, PCWSTR::from_raw(line.as_ptr()), -1);
                } else {
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                    let mut hint = rc;
                    DrawTextW(
                        hdc,
                        w!(
                            "Open the Palette menu: every item is painted by this program (WM_MEASUREITEM sizes it, WM_DRAWITEM paints it, ODS_SELECTED drives the hover)."
                        ),
                        -1,
                        &mut hint,
                        (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
                    );
                }

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
