use windows::{
    Win32::{MB_ICONINFORMATION, MB_OK, MessageBoxW},
    core::w,
};

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
