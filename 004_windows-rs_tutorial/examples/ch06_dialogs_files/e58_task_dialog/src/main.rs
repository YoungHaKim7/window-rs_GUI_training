//! e58 — Task Dialog: the modern MessageBox, configured by struct
//!
//! What you learn:
//! * `TaskDialogIndirect` — one call, one `TASKDIALOGCONFIG` struct: title,
//!   main instruction (the big line), content (the small line), buttons,
//!   verification checkbox, callback — everything MessageBox can't do
//! * **Command links** (`TDF_USE_COMMAND_LINKS`): the buttons render as
//!   big arrow links with an optional note line after `\n` — the
//!   "Where do you want to go today?" pattern
//! * The **verification checkbox**: just set `pszVerificationText` — there
//!   is no TDF flag to show it — and read the answer afterwards through
//!   `pfVerificationFlaggedChecked` (a *mut BOOL out-parameter; preset it
//!   checked with `TDF_VERIFICATION_FLAG_CHECKED`, which DOES exist)
//! * The **callback** (`pfCallback` + `lpCallbackData`): notifications like
//!   `TDN_BUTTON_CLICKED` (wparam = the button id). Returning S_OK allows
//!   the click to close the dialog; S_FALSE would veto it. lpCallbackData
//!   is your isize pocket-luggage — we smuggle the main HWND through it.
//!
//! New Win32 APIs: `TaskDialogIndirect`, `TASKDIALOGCONFIG`,
//! `TASKDIALOG_BUTTON`, `TDF_USE_COMMAND_LINKS`, `TDN_BUTTON_CLICKED`.
//!
//! Run: `cargo run -p e58_task_dialog`

use windows::{Win32::*, core::*};

/// Main-window button that pops the task dialog.
const IDC_ASK: i32 = 1001;
/// Command-link ids (you choose; they come back via pnButton).
const IDB_SAVE: i32 = 101;
const IDB_DISCARD: i32 = 102;

#[derive(Default)]
struct State {
    last_clicked: i32, // what the callback saw last
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
            lpszClassName: w!("e58_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e58_class"),
            w!("e58 · Task Dialog — the modern question"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            500,
            240,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let ask = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Ask me anything"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            30,
            150,
            32,
            Some(hwnd),
            Some(IDC_ASK as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!ask.is_null());

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

/// The task-dialog callback: notification code in msg, payload in wparam.
/// lprefdata is the lpCallbackData we put in the config — here the main
/// window's HWND, so the callback can talk back to our state.
extern "system" fn callback(
    _hwnd: HWND,
    notification: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
    lprefdata: isize,
) -> HRESULT {
    unsafe {
        if notification == TDN_BUTTON_CLICKED as u32 {
            // wparam = the clicked button id (ours, or IDCANCEL=2 for the
            // auto-added cancel). Record it on the main window.
            if let Some(state) = state_of(lprefdata as HWND) {
                state.last_clicked = wparam as i32;
            }
        }
        // S_OK: "notification handled, proceed" (for TDN_BUTTON_CLICKED
        // that means: close). Returning S_FALSE instead would VETO the
        // close — handy for "you can't discard unsaved work" logic.
        HRESULT(0)
    }
}

/// Configure and run the task dialog; returns (clicked id, checkbox state).
unsafe fn run_task_dialog(owner: HWND) -> (i32, bool) {
    unsafe {
        // Command-link texts support a two-line form: "headline\nnote".
        let buttons = [
            TASKDIALOG_BUTTON {
                nButtonID: IDB_SAVE,
                pszButtonText: w!("Save and close\nKeep today's edits"),
            },
            TASKDIALOG_BUTTON {
                nButtonID: IDB_DISCARD,
                pszButtonText: w!("Discard and close\nThrow the edits away"),
            },
        ];

        let mut config = TASKDIALOGCONFIG {
            cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
            ..Default::default()
        };
        config.hwndParent = owner;
        config.pszWindowTitle = w!("Unsaved changes"); // caption bar
        config.pszMainInstruction = w!("What should happen to your document?"); // the big line
        config.pszContent = w!("The choice is remembered in the checkbox below — \
                                that's the verification feature.");
        config.pButtons = buttons.as_ptr();
        config.cButtons = buttons.len() as u32;
        config.nDefaultButton = IDB_SAVE; // Enter picks this one
        // TDF_USE_COMMAND_LINKS: render pButtons as big links instead of
        // plain buttons. TDF_ALLOW_DIALOG_CANCELLATION: Esc/[X] = cancel
        // even without a TDCBF_CANCEL_BUTTON (we add one anyway below).
        config.dwFlags = TDF_USE_COMMAND_LINKS | TDF_ALLOW_DIALOG_CANCELLATION;
        config.dwCommonButtons = TDCBF_CANCEL_BUTTON; // auto-made Cancel
        // Setting pszVerificationText is ALONE what shows the checkbox
        // (there is no TDF_VERIFICATION_FLAGGED constant in this crate —
        // only TDF_VERIFICATION_FLAG_CHECKED, which would pre-tick it).
        config.pszVerificationText = w!("Remember my choice");
        config.pfCallback = Some(callback);
        config.lpCallbackData = owner as isize; // smuggled to the callback

        let mut selection = 0i32; // out: the clicked button id
        let mut verified = BOOL::default(); // out: checkbox state
        TaskDialogIndirect(
            &config,
            Some(&mut selection),
            None, // out: radio button (not used here)
            Some(&mut verified),
        )
        .ok()
        .expect("TaskDialogIndirect failed");

        (selection, verified.0 != 0)
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
                if notification == BN_CLICKED as u32 && control_id == IDC_ASK as u32 {
                    let (selection, verified) = run_task_dialog(hwnd);
                    let what = match selection {
                        IDB_SAVE => "save",
                        IDB_DISCARD => "discard",
                        IDCANCEL => "cancel",
                        _ => "???",
                    };
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!(
                            "e58 · chose {what} — checkbox {} (callback last saw id {})",
                            if verified { "ticked" } else { "unticked" },
                            state.last_clicked
                        )),
                    );
                }
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
