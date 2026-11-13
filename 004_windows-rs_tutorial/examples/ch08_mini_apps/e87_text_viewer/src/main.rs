//! e87 — Text Viewer: scrollbars, metrics, a modeless Find, highlights
//!
//! What you learn:
//! * **Text by hand**: no EDIT control — lines are `Vec<String>`, laid
//!   out in WM_PAINT with `TextOutW`. The line height comes from
//!   `GetTextMetricsW` (tmHeight of the SELECTED font — measure, never
//!   guess 16), which is also the page unit for the scrollbar math.
//! * Window scrollbars, the e28 pattern condensed: `SetScrollInfo` with
//!   range = line count, page = visible lines, position = first visible
//!   line; WM_VSCROLL + WM_MOUSEWHEEL move it. (Simplification vs e28:
//!   full repaint instead of ScrollWindowEx — fine for a viewer, and
//!   the highlights make full repaints the honest choice anyway.)
//! * **The modeless Find dialog**, built from an in-memory template
//!   (e55/e56): `CreateDialogIndirectParamW` + `IsDialogMessageW` in
//!   the pump + close-by-DestroyWindow. The dialog writes the query
//!   straight into the OWNER's state via `GetParent` (e56's trick).
//! * **Highlighting**: case-insensitive search over lines; matches get
//!   a yellow bar drawn by measuring the prefix and match widths with
//!   `GetTextExtentPoint32W`, FillRect, then one transparent TextOutW.
//! * F3 = next match: recompute nothing, just advance an index and
//!   `SetScrollPos` + `InvalidateRect` — the scrollbar and the paint
//!   both read the same `top_line`.
//!
//! File → Open loads any text file; Search → Find (F3 cycles matches).
//!
//! Run: `cargo run -p e87_text_viewer`

use windows::{Win32::*, core::*};

const IDM_OPEN: usize = 4001;
const IDM_EXIT: usize = 4002;
const IDM_FIND: usize = 4003;

/// The Find dialog's EDIT control.
const IDC_FIND_EDIT: i32 = 2001;

const TIMER_ID: usize = 0; // unused; keeps shape symmetric with siblings

#[derive(Default)]
struct State {
    lines: Vec<String>,
    top_line: i32,
    line_height: i32, // from GetTextMetricsW, 0 until first paint
    /// Modeless Find dialog (null while closed) — the pump tests it.
    hwnd_find: HWND,
    /// Current query ("" = none) and the F3 cursor into matches.
    query: String,
    match_lines: Vec<usize>,
    next_match: usize,
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
            lpszClassName: w!("e87_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e87_class"),
            w!("e87 · Text Viewer — open a file, then Search → Find"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_VSCROLL) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            520,
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
            // e56: while the modeless Find dialog exists, give it first
            // crack at messages (Tab, Enter→IDOK, Esc→IDCANCEL...).
            let dialog_open = {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                !ptr.is_null() && !(*ptr).hwnd_find.is_null()
            };
            if dialog_open && IsDialogMessageW(find_dialog_of(hwnd), &mut message).0 != 0 {
                continue;
            }
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// The dialog HWND lives in the MAIN window's state — fetch it for the pump.
unsafe fn find_dialog_of(hwnd: HWND) -> HWND {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            std::ptr::null_mut()
        } else {
            (*ptr).hwnd_find
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

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Push range/page/position for the vertical bar (e28's set_bar).
unsafe fn update_scrollbar(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let page = if state.line_height > 0 {
            (rc.bottom / state.line_height).max(1)
        } else {
            1
        };
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: (SIF_RANGE | SIF_PAGE | SIF_POS) as u32,
            nMin: 0,
            nMax: state.lines.len().max(1) as i32 - 1,
            nPage: page as u32,
            nPos: state.top_line,
            nTrackPos: 0,
        };
        _ = SetScrollInfo(hwnd, SB_VERT, &si, true);
    }
}

/// Recompute matching line indexes for the current query.
unsafe fn recompute_matches(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        state.match_lines.clear();
        state.next_match = 0;
        if state.query.is_empty() {
            return;
        }
        let needle = state.query.to_lowercase();
        for (i, line) in state.lines.iter().enumerate() {
            if line.to_lowercase().contains(&needle) {
                state.match_lines.push(i);
            }
        }
    }
}

/// Scroll so line `line` is comfortably visible, sync the bar, repaint.
unsafe fn scroll_to_line(hwnd: HWND, line: i32) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let page = if state.line_height > 0 {
            (rc.bottom / state.line_height).max(1)
        } else {
            1
        };
        // Center the target line in the viewport (classic viewer move).
        state.top_line = (line - page / 2).clamp(0, (state.lines.len() as i32 - page).max(0));
        _ = SetScrollPos(hwnd, SB_VERT, state.top_line, true);
        _ = InvalidateRect(Some(hwnd), None, true);
    }
}

/// Advance to the next match (F3 / Find Next).
unsafe fn goto_next_match(hwnd: HWND) {
    unsafe {
        let target = {
            let Some(state) = state_of(hwnd) else { return };
            if state.match_lines.is_empty() {
                None
            } else {
                let idx = state.next_match % state.match_lines.len();
                state.next_match = (state.next_match + 1) % state.match_lines.len();
                Some(state.match_lines[idx] as i32)
            }
        };
        if let Some(line) = target {
            scroll_to_line(hwnd, line);
        }
    }
}

// ======================================================================
// The in-memory dialog template builder — e55's machinery verbatim.
// ======================================================================
struct TemplateBuilder {
    words: Vec<u16>,
    start: usize,
    pos: usize,
}

impl TemplateBuilder {
    fn new() -> Self {
        let words = vec![0u16; 256];
        let start = (!(words.as_ptr() as usize).is_multiple_of(4)) as usize;
        Self {
            words,
            start,
            pos: start,
        }
    }
    fn word(&mut self, w: u16) {
        self.words[self.pos] = w;
        self.pos += 1;
    }
    fn dword(&mut self, dw: u32) {
        self.word(dw as u16);
        self.word((dw >> 16) as u16);
    }
    fn i16s(&mut self, values: &[i16]) {
        for &v in values {
            self.word(v as u16);
        }
    }
    fn string(&mut self, s: &str) {
        for u in s.encode_utf16() {
            self.word(u);
        }
        self.word(0);
    }
    fn align_dword(&mut self) {
        if self.pos % 2 == 1 {
            self.word(0);
        }
    }
    fn as_lpcdlgtemplate(&self) -> LPCDLGTEMPLATEW {
        unsafe { self.words.as_ptr().add(self.start) as LPCDLGTEMPLATEW }
    }
}

/// Dialog: an EDIT + "Find next" (IDOK) + "Close" (IDCANCEL).
fn build_find_template() -> TemplateBuilder {
    let mut t = TemplateBuilder::new();

    // DLGTEMPLATE header.
    t.dword(WS_POPUP | (WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_CENTER) as u32);
    t.dword(0);
    t.word(3); // three items: EDIT + two buttons
    t.i16s(&[0, 0, 190, 64]); // dialog units
    t.word(0); // no menu
    t.word(0); // class: #32770 dialog manager
    t.string("Find");

    // Item 1: the EDIT (class atom 0x0081).
    t.align_dword();
    t.dword((WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER) as u32);
    t.dword(0);
    t.i16s(&[10, 12, 170, 12]);
    t.word(IDC_FIND_EDIT as u16);
    t.word(0xFFFF);
    t.word(0x0081); // EDIT
    t.string("");
    t.word(0);

    // Item 2: Find next = IDOK (Enter triggers it too).
    t.align_dword();
    t.dword((WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON) as u32);
    t.dword(0);
    t.i16s(&[10, 34, 80, 14]);
    t.word(IDOK as u16);
    t.word(0xFFFF);
    t.word(0x0080); // BUTTON
    t.string("Find next");
    t.word(0);

    // Item 3: Close = IDCANCEL (Esc).
    t.align_dword();
    t.dword((WS_CHILD | WS_VISIBLE | WS_TABSTOP) as u32);
    t.dword(0);
    t.i16s(&[100, 34, 80, 14]);
    t.word(IDCANCEL as u16);
    t.word(0xFFFF);
    t.word(0x0080);
    t.string("Close");
    t.word(0);

    t
}

extern "system" fn find_dlgproc(
    hdlg: HWND,
    message: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> INT_PTR {
    unsafe {
        match message as i32 {
            WM_INITDIALOG => {
                // Seed the edit with the previous query, if any.
                if let Some(state) = state_of(GetParent(hdlg))
                    && !state.query.is_empty() {
                    let edit = GetDlgItem(Some(hdlg), IDC_FIND_EDIT);
                    _ = SetWindowTextW(edit, &HSTRING::from(state.query.clone()));
                }
                1 // "set focus yourself"
            }
            WM_COMMAND => {
                match (wparam & 0xffff) as u32 as i32 {
                    IDOK => {
                        // Read the edit, push the query into the OWNER's
                        // state (e56's cross-window pattern), recompute,
                        // jump. The owner is GetParent(hdlg).
                        let owner = GetParent(hdlg);
                        let mut buf = [0u16; 128];
                        let got =
                            GetDlgItemTextW(hdlg, IDC_FIND_EDIT, PWSTR(buf.as_mut_ptr()), 128);
                        let query = wide_to_string(&buf[..got as usize]);
                        if let Some(state) = state_of(owner) {
                            state.query = query;
                        }
                        recompute_matches(owner);
                        goto_next_match(owner);
                        1
                    }
                    IDCANCEL => {
                        _ = DestroyWindow(hdlg);
                        1
                    }
                    _ => 0,
                }
            }
            WM_CLOSE => {
                _ = DestroyWindow(hdlg);
                1
            }
            WM_DESTROY => {
                // Modeless = DestroyWindow, and clear the pump's flag
                // (the HWND lives in the OWNER's state).
                let owner = GetParent(hdlg);
                if let Some(state) = state_of(owner) {
                    state.hwnd_find = std::ptr::null_mut();
                }
                0
            }
            _ => 0, // no DefWindowProc in a DLGPROC (e55)
        }
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
                let bar = CreateMenu();
                let file_menu = CreatePopupMenu();
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_OPEN, w!("&Open..."));
                _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_EXIT, w!("E&xit"));
                _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
                let search_menu = CreatePopupMenu();
                _ = AppendMenuW(
                    search_menu,
                    MF_STRING as u32,
                    IDM_FIND,
                    w!("&Find...\tF3-ready"),
                );
                _ = AppendMenuW(bar, MF_POPUP as u32, search_menu as usize, w!("&Search"));
                _ = SetMenu(hwnd, Some(bar));

                // Opening words so the viewer is never blank.
                if let Some(state) = state_of(hwnd) {
                    state.lines = vec![
                        String::from("Open a file (File > Open...) or search this page first."),
                        String::from(""),
                        String::from("Search > Find opens a MODELESS dialog built from an"),
                        String::from("in-memory DLGTEMPLATE (e55/e56). Find next jumps; F3"),
                        String::from("cycles matches. Highlights are drawn by measuring"),
                        String::from("prefix and match widths with GetTextExtentPoint32W."),
                    ];
                }
                update_scrollbar(hwnd);
                0
            }

            WM_SIZE => {
                update_scrollbar(hwnd);
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }

            WM_VSCROLL => {
                // e28 condensed: compute new top, clamp, apply, repaint.
                let mut si = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL as u32,
                    ..Default::default()
                };
                _ = GetScrollInfo(hwnd, SB_VERT, &mut si);
                let old = si.nPos;
                let max_pos = (si.nMax - si.nPage as i32 + 1).max(si.nMin);
                let new = match (wparam & 0xffff) as i32 {
                    SB_LINEUP => old - 1,
                    SB_LINEDOWN => old + 1,
                    SB_PAGEUP => old - si.nPage as i32,
                    SB_PAGEDOWN => old + si.nPage as i32,
                    SB_TOP => si.nMin,
                    SB_BOTTOM => max_pos,
                    SB_THUMBTRACK | SB_THUMBPOSITION => si.nTrackPos,
                    _ => old,
                }
                .clamp(si.nMin, max_pos);
                if new != old {
                    if let Some(state) = state_of(hwnd) {
                        state.top_line = new;
                    }
                    _ = SetScrollPos(hwnd, SB_VERT, new, true);
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }

            WM_MOUSEWHEEL => {
                // Three lines per notch, reusing the scroll arm above.
                let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
                let code = if delta < 0 { SB_LINEDOWN } else { SB_LINEUP };
                for _ in 0..3 {
                    _ = SendMessageW(hwnd, WM_VSCROLL as u32, code as WPARAM, 0);
                }
                0
            }

            WM_KEYDOWN => {
                // F3 = next match, no dialog needed.
                if wparam as i32 == VK_F3 {
                    goto_next_match(hwnd);
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }

            WM_COMMAND => {
                let id = wparam & 0xffff;
                match id {
                    IDM_OPEN => {
                        // e59 picker, condensed.
                        let mut blob = Vec::new();
                        for part in [
                            "Text files (*.txt;*.md;*.rs)",
                            "*.txt;*.md;*.rs",
                            "All files (*.*)",
                            "*.*",
                        ] {
                            blob.extend(part.encode_utf16());
                            blob.push(0);
                        }
                        blob.push(0);
                        let mut file_buf = vec![0u16; 260];
                        let mut ofn = OPENFILENAMEW {
                            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                            hwndOwner: hwnd,
                            lpstrFilter: PCWSTR::from_raw(blob.as_ptr()),
                            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
                            nMaxFile: file_buf.len() as u32,
                            nFilterIndex: 1,
                            Flags: (OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST) as u32,
                            ..Default::default()
                        };
                        if GetOpenFileNameW(&mut ofn).0 != 0 {
                            let path = wide_to_string(&file_buf);
                            match std::fs::read_to_string(&path) {
                                Ok(text) => {
                                    // Split; trim the CR CRLF leaves behind.
                                    let lines: Vec<String> = text
                                        .lines()
                                        .map(|l| l.trim_end_matches('\r').to_string())
                                        .collect();
                                    if let Some(state) = state_of(hwnd) {
                                        state.lines = lines;
                                        state.top_line = 0;
                                    }
                                    recompute_matches(hwnd);
                                    update_scrollbar(hwnd);
                                    _ = InvalidateRect(Some(hwnd), None, true);
                                    _ = SetWindowTextW(hwnd, &HSTRING::from(path));
                                }
                                Err(err) => {
                                    _ = MessageBoxW(
                                        Some(hwnd),
                                        &HSTRING::from(format!("Could not read:\n{err}")),
                                        w!("e87"),
                                        (MB_OK | MB_ICONERROR) as u32,
                                    );
                                }
                            }
                        }
                        0
                    }

                    IDM_FIND => {
                        // Only one instance at a time.
                        let already_open = state_of(hwnd)
                            .map(|s| !s.hwnd_find.is_null())
                            .unwrap_or(false);
                        if !already_open {
                            let template = build_find_template();
                            let hdlg = CreateDialogIndirectParamW(
                                Some(GetModuleHandleW(PCWSTR::null())),
                                template.as_lpcdlgtemplate(),
                                Some(hwnd),
                                Some(find_dlgproc),
                                0,
                            );
                            if let Some(state) = state_of(hwnd) {
                                state.hwnd_find = hdlg;
                            }
                            _ = ShowWindow(hdlg, SW_SHOW);
                        }
                        0
                    }

                    IDM_EXIT => {
                        _ = DestroyWindow(hwnd);
                        0
                    }

                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // Font metrics ONCE per paint (cheap); the line height
                // must come from the font, not a guess.
                let mut tm = TEXTMETRICW::default();
                _ = GetTextMetricsW(hdc, &mut tm);
                let line_h = tm.tmHeight.max(1);

                if let Some(state) = state_of(hwnd) {
                    state.line_height = line_h;
                    let page = rc.bottom / line_h;

                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, 0x00202020);

                    let needle = state.query.to_lowercase();
                    for row in 0..page {
                        let index = state.top_line + row;
                        if index >= state.lines.len() as i32 {
                            break;
                        }
                        let line = &state.lines[index as usize];
                        let y = row * line_h;

                        // Draw the line, then lay highlight bars under
                        // every case-insensitive occurrence.
                        let wide = to_wide(line);
                        if !needle.is_empty() {
                            let lower = line.to_lowercase();
                            let mut from = 0;
                            while let Some(found) = lower[from..].find(&needle) {
                                let start = from + found;
                                let prefix = &line[..start];
                                let hit = &line[start..start + needle.len()];
                                // Measure with the CURRENT dc font —
                                // widths match what TextOutW is about to
                                // draw, so the bar sits exactly.
                                let mut size = SIZE::default();
                                let pre = to_wide(prefix);
                                _ = GetTextExtentPoint32W(
                                    hdc,
                                    PCWSTR::from_raw(pre.as_ptr()),
                                    pre.len() as i32 - 1, // minus terminator
                                    &mut size,
                                );
                                let mut hit_size = SIZE::default();
                                let hw = to_wide(hit);
                                _ = GetTextExtentPoint32W(
                                    hdc,
                                    PCWSTR::from_raw(hw.as_ptr()),
                                    hw.len() as i32 - 1,
                                    &mut hit_size,
                                );
                                let brush = CreateSolidBrush(0x00FF_FF00); // yellow (BGR)
                                _ = FillRect(
                                    hdc,
                                    &RECT {
                                        left: 4 + size.cx,
                                        top: y,
                                        right: 4 + size.cx + hit_size.cx,
                                        bottom: y + line_h,
                                    },
                                    brush,
                                );
                                _ = DeleteObject(brush as HGDIOBJ);
                                from = start + needle.len().max(1);
                            }
                        }

                        _ = TextOutW(hdc, 4, y, PCWSTR::from_raw(wide.as_ptr()), -1);
                    }

                    // Status line at the bottom of the frame: matches.
                    if !state.query.is_empty() {
                        let report = if state.match_lines.is_empty() {
                            format!("no matches for \"{}\"", state.query)
                        } else {
                            format!(
                                "{} matching line(s) for \"{}\" — F3: next",
                                state.match_lines.len(),
                                state.query
                            )
                        };
                        _ = SetTextColor(hdc, 0x00808080);
                        let wide = to_wide(&report);
                        _ = TextOutW(
                            hdc,
                            4,
                            rc.bottom - line_h,
                            PCWSTR::from_raw(wide.as_ptr()),
                            -1,
                        );
                    }
                }

                _ = EndPaint(hwnd, &ps);
                update_scrollbar(hwnd);
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
                let _ = TIMER_ID;
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
