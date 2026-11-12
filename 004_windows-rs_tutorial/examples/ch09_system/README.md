# Chapter 9 — System Interop

The examples so far stayed inside one window; this chapter points the same
windows-rs bindings *outward* at the rest of the operating system: the
clipboard every app shares, the pixels currently on your monitors, the
process table, the machine's vitals, the registry, worker threads, and
every other top-level window on the desktop. The recurring lesson is that
all of these are just more HANDLE-and-struct APIs — once the open / use /
close (or callback / lparam) shapes from earlier chapters are muscle
memory, "talking to the system" is more of the same. Two patterns get a
formal introduction here: the push-style enumeration callback (EnumWindows,
e95) beside the pull-style snapshot walk (e91), and the worker-thread →
PostMessageW mailbox that keeps a GUI responsive during real work (e94).

- **e89** `e89_clipboard` — open/empty/set/close the clipboard; `GMEM_MOVEABLE`
  global-heap text in `CF_UNICODETEXT`, system ownership after
  `SetClipboardData`, and the paste path back out
- **e90** `e90_screen_capture` — `GetDC(None)` on the whole desktop, one cached
  `CreateCompatibleBitmap` snapshot (re-take with R), `StretchBlt` to fit,
  and the e32 .bmp writer reused to save it (S)
- **e91** `e91_process_list` — `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` plus
  the `Process32FirstW`/`Process32NextW` walk; `dwSize`-first and
  fixed-array-string gotchas, in a listbox with Refresh
- **e92** `e92_system_info` — `GetSystemInfo` (anonymous unions in Rust!),
  `GlobalMemoryStatusEx` (set `dwLength` first), `GetSystemMetrics`, and
  `GetTickCount64` drawn as a text table
- **e93** `e93_registry_settings` — a window that remembers its position/size in
  `HKCU\Software\windows-rs-tutorial\e93`: `RegCreateKeyExW`,
  `RegQueryValueExW` with ERROR_FILE_NOT_FOUND as first-run detection,
  `RegSetValueExW` at WM_DESTROY, `RegCloseKey`
- **e94** `e94_worker_thread` — "Generate..." offloads chunked work to a
  `std::thread` that reports back via `PostMessageW(WM_APP+n)`; hwnd smuggled
  as `isize` (raw HWND is `!Send`), progress bar, cooperative cancel via
  `Arc<AtomicBool>`
- **e95** `e95_enum_windows` — `EnumWindows` with an `extern "system"` callback
  (nonzero = continue), context through `lparam *mut Vec`, visible+titled
  filter, listbox, and double-click `SetForegroundWindow` (with the
  foreground-lock caveat)

**Suggested order:** numeric — e89→e95 mirror the chapter's arc from "shared
system resources" to "threads and enumeration".
