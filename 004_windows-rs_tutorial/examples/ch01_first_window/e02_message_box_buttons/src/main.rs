//! e02 — Message Box Buttons & Icons: asking the user a question
//!
//! What you learn:
//! * `MessageBoxW` is *modal*: it blocks until the user clicks a button
//! * Combining button layouts (`MB_YESNOCANCEL`) with icons (`MB_ICONWARNING`)
//! * Reading the answer via the return value (`IDYES`, `IDNO`, `IDCANCEL`)
//! * `MB_DEFBUTTON2`: choosing which button Enter presses by default
//!
//! New Win32 APIs:
//! * `MessageBoxW` — again, but this time we care what it returns.
//!
//! Run: `cargo run -p e02_message_box_buttons`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        // The u32 argument is a bit field: pick ONE button layout, ONE icon,
        // and any modifiers, OR-ed together.
        let choice = MessageBoxW(
            None, // no owner window
            w!("Save your changes before exiting?"),
            w!("e02 · Buttons and Icons"),
            (MB_YESNOCANCEL | MB_ICONWARNING | MB_DEFBUTTON2) as u32,
        );

        // The return value tells you which button was clicked (or IDTIMEOUT
        // if you used MB_TIMEOUT... rare). Note how the terminal stays alive
        // until you answer — that's "modal".
        let answer = match choice {
            IDYES => "Yes     — pretend we saved your work",
            IDNO => "No      — discard the changes",
            IDCANCEL => "Cancel  — keep the app running",
            _ => "Unexpected return value!",
        };
        println!("MessageBoxW returned {choice}: {answer}");

        // Same function, different costume: an "error" box with a single OK.
        // Escape and Enter both dismiss it (MB_OK has only one outcome).
        MessageBoxW(
            None,
            w!(
                "That was a MessageBoxW with:\n\u{2022} MB_YESNOCANCEL + MB_ICONWARNING\n\u{2022} MB_DEFBUTTON2 (No was the default)\n\u{2022} and this one: MB_OK + MB_ICONINFORMATION"
            ),
            w!("Recap"),
            (MB_OK | MB_ICONINFORMATION) as u32,
        );
    }
}
