# window-rs_GUI_training

- https://github.com/microsoft/windows-rs/tree/master/crates/samples

# link

- [Windows공식문서 API공부하기](#windows-api문서)
  - [윈도우GUI관련 소스 wine소스를 보자windows.h](#windowsh)

<hr>

# 여기에 정리중.
- https://github.com/YoungHaKim7/window-rs_GUI_training

<hr />

# windows-rs

- https://github.com/microsoft/windows-rs

- tutorial eBook
  - https://kennykerr.ca/rust-getting-started/

<hr>

# windows-rs
- windows-rs![icedstar](https://img.shields.io/github/stars/microsoft/windows-rs.svg)

    - Rust for Windows
      - https://kennykerr.ca/rust-getting-started/

    - https://github.com/microsoft/windows-rs
    - This repo is the home of the following crates (and other supporting crates):

        - windows-sys - Raw bindings for C-style Windows APIs.
        - windows - Safer bindings including C-style APIs as well as COM and WinRT APIs.
        - windows-core - Type support for the windows crate.
        - windows-implement - The implement macro for the windows crate, for implementing COM interfaces.
        - windows-interface - The interface macro for the windows crate, for declaring COM interfaces.
        - windows-targets - Import libs for Windows.
        - windows-version - Windows version information.
        - windows-metadata - Windows metadata reader.
        - windows-bindgen - Windows metadata compiler library.
        - riddle - Windows metadata compiler tool.

<hr>

# Windows API문서
- https://github.com/MicrosoftDocs/win32
  - windows-rs feature search
    - Type a metadata API name to find the Cargo feature(s) to enable in the windows or windows-sys crate. Rich bindings may project some metadata types to sibling crates such as windows-numerics. A method or function may need more than one feature when its parameters span namespaces. Matching is a case-insensitive regular expression over Namespace::Name.
    - https://microsoft.github.io/windows-rs/features/

- User mode and kernel mode
  - https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode

# windows.h
- https://github.com/wine-mirror/wine
  - 원본(wine, Linux에서 Windows프로그램 돌리기)
    - https://gitlab.winehq.org/wine/wine

# gui공부자료
- https://users.rust-lang.org/t/gui-or-2d-game-crate-writing-some-drawing-app/79169
- https://www.reddit.com/r/rust/comments/5j07bx/how_to_draw_a_pixel_to_the_screen/

# 윈도우OS dev) WindowsOS GUI 만들때 이거 쓰자


```toml
[dependencies]
windows-sys = { version = "0.59", features = [
  "Win32_Foundation", "Win32_Graphics_Gdi", "Win32_Graphics_Dwm",
  "Win32_Networking_WinSock", "Win32_NetworkManagement_IpHelper", "Win32_NetworkManagement_NetManagement",
  "Win32_Security", "Win32_Security_Authorization", "Win32_Security_Cryptography",
  "Win32_Storage_FileSystem", "Win32_System_Com", "Win32_System_Console",
  "Win32_System_DataExchange", "Win32_System_Diagnostics_Debug",
  "Win32_System_Environment", "Win32_System_IO", "Win32_System_JobObjects",
  "Win32_System_LibraryLoader", "Win32_System_Memory", "Win32_System_Ole",
  "Win32_System_Pipes", "Win32_System_Power", "Win32_System_Registry", "Win32_System_SystemInformation",
  "Win32_System_SystemServices", "Win32_System_Threading", "Win32_System_Variant",
  "Win32_UI_Accessibility", "Win32_UI_Controls", "Win32_UI_Controls_Dialogs", "Win32_UI_HiDpi",
  "Win32_UI_Input_KeyboardAndMouse", "Win32_UI_Shell", "Win32_UI_Shell_Common", "Win32_UI_WindowsAndMessaging"
] }


[build-dependencies]
embed-resource = "3.0"

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "unwind"
debug = 1
strip = "none"
```

- https://github.com/isolmaz/SSDownload


# 외국인의 작품
- https://github.com/JohnScience/draw_on_screen

