# Chapter 4 — Menus

Menus are the classic Windows command surface, and this chapter covers
them end to end: building a bar in code, context menus, and items that
carry state (check marks, radio groups, graying). Menus turn out to be
live objects you can grow and prune at runtime, a keyboard parallel
exists in accelerator tables, the system menu is yours to extend — and
if the built-in look isn't enough, owner-draw lets you paint every item
yourself. Throughout, the same loop repeats: menu choices mutate window
state (the e13 pattern), and WM_COMMAND is the single door they all
walk through.

| # | Example | What you learn |
|---|---------|----------------|
| e33 | `e33_menu_bar` | `CreateMenu`/`AppendMenuW`/`SetMenu`; picks arrive as `WM_COMMAND` with the id in LOWORD(wparam) |
| e34 | `e34_context_menu` | `WM_CONTEXTMENU` (right-click *and* Shift+F10), `TrackPopupMenu`, and `TPM_RETURNCMD` vs the WM_COMMAND alternative |
| e35 | `e35_menu_state` | `CheckMenuItem` ✓ toggles, `CheckMenuRadioItem` bullet groups, `EnableMenuItem` graying — mirrored into painted text |
| e36 | `e36_dynamic_menu` | A "Recent" submenu grown by `AppendMenuW`, capped at 5, oldest evicted via `RemoveMenu` — `MF_BYPOSITION` vs `MF_BYCOMMAND` |
| e37 | `e37_accelerators` | `CreateAcceleratorTableW` + `TranslateAcceleratorW` first in the pump; ids shared between menu items and Ctrl+key shortcuts |
| e38 | `e38_system_menu` | `GetSystemMenu(hwnd, FALSE)` + custom items; `WM_SYSCOMMAND` with the `& 0xFFF0` mask; the `TRUE` reset |
| e39 | `e39_runtime_icon` | A 32×32 icon built in code (`CreateBitmap` mask + color planes, `ICONINFO`, `CreateIconIndirect`) and applied live via `WM_SETICON` |
| e40 | `e40_owner_draw_menu` | `MF_OWNERDRAW` items: `WM_MEASUREITEM` sizes them, `WM_DRAWITEM` paints swatch + label, `ODS_SELECTED` drives the hover |

**Suggested order:** straight through — e33's WM_COMMAND handling is
assumed everywhere after.
