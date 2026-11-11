# Chapter 6 — Dialogs & Files

Dialogs are the second face of Win32 windows: created from *templates*
rather than a stream of `CreateWindowExW` calls, with their own dialog
procedure and free keyboard navigation. This tutorial builds every dialog
template **in memory** (no `.rc` resource scripts — everything stays pure
Rust), then tours the common dialogs: open/save files, colors, fonts,
folders, and the modern task dialog.

| # | Example | What you learn |
|---|---------|----------------|
| e55 | `e55_modal_dialog` | The in-memory `DLGTEMPLATE` recipe (word alignment!), `DialogBoxIndirectParamW`, `WM_INITDIALOG`, `EndDialog` |
| e56 | `e56_modeless_dialog` | `CreateDialogIndirectParamW` + `IsDialogMessage` in the pump — Tab/Enter handling for free |
| e57 | `e57_dialog_controls` | A real Settings dialog (edit + checkbox + radios) passing data both ways via `dwInitParam` |
| e58 | `e58_task_dialog` | The modern `TaskDialogIndirect`: command links, verification checkbox, callbacks |
| e59 | `e59_open_file_dialog` | `GetOpenFileNameW`: the double-null-terminated filter format, `OFN_*` flags |
| e60 | `e60_save_file_dialog` | `GetSaveFileNameW`: overwrite prompts, default extensions — and actually writing the file |
| e61 | `e61_color_picker` | `ChooseColorW`: custom color palettes that persist between opens |
| e62 | `e62_font_picker` | `ChooseFontW` → `LOGFONTW` → `CreateFontIndirectW`: live font preview |
| e63 | `e63_folder_browse` | `SHBrowseForFolderW`: picking directories, freeing the PIDL |
| e64 | `e64_file_io_text` | Wiring it together: open → edit → save a text file end-to-end (notepad's shadow; the real one is e77) |

**Suggested order:** e55 is the keystone — the template-building helper
reappears in e56, e57, and e87's find dialog. After that, order is free.
