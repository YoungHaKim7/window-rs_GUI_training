# Style guide for all 100 tutorial examples

Every example is a **self-contained crate** — no shared helper library. The
`RegisterClassW` / `CreateWindowExW` / message-loop boilerplate is repeated on
purpose: repetition is how the Win32 boilerplate becomes muscle memory, and it
keeps every example readable in isolation (Petzold tradition).

## Crate layout

```
eNN_name/
├── Cargo.toml
└── src/main.rs
```

`Cargo.toml` (copy this shape, adjust name and features):

```toml
[package]
name = "eNN_name"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies.windows]
workspace = true
features = ["winuser"]          # only the features this example needs
```

Run with: `cargo run -p eNN_name` (from the workspace root).

## main.rs shape

```rust
//! eNN — Title in Title Case
//!
//! What you learn:
//! * bullet list of the 1–3 concepts this example teaches
//!
//! New Win32 APIs: `FooW`, `BarW` — one line each on what they do.
//!
//! Run: `cargo run -p eNN_name`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        // ... register class, create window, message loop
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {   // WM_* constants are i32 in this crate version
            WM_PAINT => { /* ... */ 0 }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
```

## Rules

1. **Imports**: always `use windows::{Win32::*, core::*};` — the `Win32` module
   re-exports the contents of every enabled feature, so one glob covers all
   narrow features the crate enables. Only qualify deeper
   (`windows::Win32::winuser::...`) to resolve a rare ambiguity.
2. **String literals**: `w!("...")` for compile-time UTF-16 (`PCWSTR`),
   `s!("...")` for `PCSTR`. For runtime strings, build a `Vec<u16>` and pass
   `PCWSTR::from_raw(buf.as_ptr())` (keep the Vec alive!) or use `HSTRING`.
3. **Safety**: `fn main()` with an inner `unsafe { }` block — not `unsafe fn`.
   Never `unsafe impl`. Keep `unsafe` scopes reasonably tight.
4. **wndproc** is `extern "system"`, matches on `message as i32` (WM constants
   are `i32` here), returns `LRESULT`, and always falls through to
   `DefWindowProcW`.
5. **Window creation defaults** (so examples stay comparable):
   class style `CS_HREDRAW | CS_VREDRAW`, cursor `LoadCursorW(None, IDC_ARROW)?`
   — actually `IDC_ARROW` is already a `PCWSTR`-compatible constant; background
   `(COLOR_WINDOW + 1) as HBRUSH`; style `WS_OVERLAPPEDWINDOW | WS_VISIBLE`;
   position/size `(CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT)`.
6. **Errors**: for one-shot apps, `debug_assert!` / `.expect(...)` on failures
   is fine and keeps the teaching flow; return `windows::core::Result<()>`
   from main when the example is about error handling. Prefer `.ok()?` when a
   call returns a `Result`.
7. **Comments**: English, address the reader as "you", explain the *why* of
   every flag or non-obvious parameter. Target density: a comment per logical
   step, not per line.
8. **No dependencies** other than the `windows` crate (exception: the two
   chapter-10 examples that use `windows-window` / `windows-webview`).
9. **Feature list**: check `windows-rs/crates/libs/windows/Cargo.toml` for the
   exact short feature names (`winuser`, `wingdi`, `commctrl`, `commdlg`,
   `dwmapi`, `shellapi`, `tlhelp32`, `winreg`, ...). When unsure about an API
   signature, grep
   `windows-rs/crates/libs/windows/src/Windows/Win32/<module>/mod.rs` — never
   guess.
10. **Verifying API usage**: every example must compile
    (`cargo build -p eNN_name`) before it counts as done.

## Crate-version idioms (learned the hard way in ch01)

* The import is `use windows::{Win32::*, core::*};` — don't forget the
  closing `}`.
* `BOOL` (and some other returns) are `#[must_use]`: when you ignore the
  result, write `_ = SomeCall(...);` — the project builds with zero
  warnings.
* **Constant types**: most `WS_*`, `WM_*`, `MB_*`, `CS_*`, `DT_*`, `VK_*`,
  `SWP_*`, `FW_*` constants are `i32`, while many parameters are `u32`.
  Bridge with a cast in parens: `(WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32`.
* **GDI object handles** (`HFONT`, `HPEN`, `HBRUSH`, `HBITMAP`, `HBRUSH`)
  are distinct pointer types, but `SelectObject`/`DeleteObject` take
  `HGDIOBJ` (`*mut c_void`): cast with `font as HGDIOBJ`.
* Handles are **raw pointer aliases** (`HWND = *mut HWND__`): test with
  `.is_null()`, not `== 0`; null handles are `None` in `Option`-taking
  parameters.
* Stock ids: `LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast()))` (the constant
  is a `PCSTR` ordinal; same dance for `IDI_APPLICATION` + `LoadIconW`).
* Background brush: `((COLOR_WINDOW + 1) as isize) as HBRUSH`.
* Class styles: `style: (CS_HREDRAW | CS_VREDRAW) as u32`.
* `MessageBoxW` flags: `(MB_OK | MB_ICONINFORMATION) as u32`.
* `GetModuleHandleW(PCWSTR::null())` returns the HINSTANCE directly
  (no `Result` in this version).
* `GetMessageW(...).0 > 0` — BOOL newtype, dereference with `.0`.
* No `RGB()` macro exists: define
  `fn rgb(r: u8, g: u8, b: u8) -> COLORREF { (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) }`.
* Runtime strings: `HSTRING::from(format!(...))` works wherever a
  `Param<PCWSTR>` is accepted; for raw `PCWSTR` parameters
  (`DrawTextW`, `TextOutW`) use a `Vec<u16>` kept alive in a local:
  `let wide = to_wide(s); TextOutW(hdc, x, y, PCWSTR::from_raw(wide.as_ptr()), -1);`
* Patterns in `match` can't contain `as` casts — but constants themselves
  ARE legal patterns: `match vk { VK_RETURN => ..., VK_ESCAPE => ... }`
  works when `vk` is an `i32`. For a mismatched scrutinee type use an arm
  guard: `_ if wparam as i32 == VK_ESCAPE`.
* **A constant "doesn't exist"?** Each constant lives in the module of the
  C header that defines it first, which is not always where you expect:
  `MOD_ALT` / `MOD_CONTROL` / `MOD_SHIFT` are in `imm` (feature `"imm"`),
  while `MOD_NOREPEAT` is in `winuser`. Grep the vendored bindings
  (`grep -rn "pub const NAME" windows-rs/crates/libs/windows/src/Windows/Win32`)
  and enable that module's feature, with a comment in Cargo.toml saying why.
* A few constants are `u32` rather than `i32` (e.g. `WS_POPUP`), so
  mixed expressions need the cast on the other operand:
  `WS_POPUP | WS_VISIBLE as u32`.
* There is no `GET_X_LPARAM`: define
  `fn mouse_pos(lparam: LPARAM) -> (i32, i32) { ((lparam & 0xffff) as i16 as i32, ((lparam >> 16) & 0xffff) as i16 as i32) }`
  (through `i16`, the coordinates are signed).

## The window-state pattern (introduced in e13, deepened in e65)

Any example that must remember something uses this — never `static mut`:

```rust
#[derive(Default)]
struct State { /* plain Rust data */ }

// in main, right after CreateWindowExW:
let state = Box::into_raw(Box::<State>::default());
SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

// first lines of wndproc:
let Some(state) = state_of(hwnd) else {
    return DefWindowProcW(hwnd, message, wparam, lparam);
};

// in WM_DESTROY, before PostQuitMessage:
let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
if !ptr.is_null() {
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
    drop(Box::from_raw(ptr));
}
```

Messages that arrive during `CreateWindowExW` (before the pointer is
attached) take the default path. If an example needs state during
creation (e.g. to create child controls in `WM_CREATE`), create the
children in `main` after attaching the state instead, or pass the pointer
through `CreateWindowExW`'s `lpparam` and attach it in `WM_NCCREATE`
(e65 demonstrates that variant).

## Working rules

* Run cargo from the workspace root using an absolute path:
  `cd "<abs path>/004_windows-rs_tutorial" && cargo build -p eNN_name`.
  Do not navigate with relative `cd ..` chains.
* Do not run the built executables: they are GUI programs that block on a
  message loop and open windows on the user's desktop. "It compiles with
  zero warnings" is the verification bar; say so honestly in your report
  and flag any runtime behavior you are unsure of.
* Fix problems with targeted edits to the one file concerned. No bulk
  `sed`/regex rewrites across multiple files.
