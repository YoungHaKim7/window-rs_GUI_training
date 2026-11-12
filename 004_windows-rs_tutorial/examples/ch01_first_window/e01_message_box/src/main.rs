//! e01 — Message Box: the smallest Windows program
//!
//! What you learn:
//! * Calling a Win32 API function (`MessageBoxW`) through the `windows` crate
//! * The `w!` macro: compile-time UTF-16 string literals for "W" functions
//! * Why every raw Win32 call in Rust lives inside `unsafe`
//!
//! New Win32 APIs:
//! * `MessageBoxW` — the classic modal dialog: shows text, waits for a click.
//!
//! Run: `cargo run -p e01_message_box`

// One glob import covers every Win32 module enabled by the crate's features
// (`Win32` re-exports the contents of each enabled feature module), and
// `core` brings the string macros (`w!`, `h!`), `PCWSTR`, `Result`, ...
use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        // Win32 functions are raw FFI calls — the compiler can't prove they're
        // memory-safe, so the `windows` crate marks them all `unsafe`.
        //
        // `w!("...")` produces a null-terminated UTF-16 string literal at
        // compile time, wrapped as a `PCWSTR` — exactly what the "W" (wide)
        // flavor of a Win32 function wants. It costs nothing at runtime.
        MessageBoxW(
            None,                            // no owner window (we don't have any window yet)
            w!("Hello, Windows from Rust!"), // body text
            w!("e01 · Message Box"),         // title bar caption
            // (MB_* style flags are i32 constants in this crate; MessageBoxW
            // takes a u32 — a plain `as` cast bridges them.)
            (MB_OK | MB_ICONINFORMATION) as u32, // an OK button + info icon
        );
    }
}
