# Chapter 2 — Messages & Input

Windows programming *is* message handling. This chapter watches the message
stream, then learns the three input surfaces: mouse, keyboard, and timers —
plus hit-testing, the shift state, hotkeys, and cursors.

The most important example here is **e13**: it introduces the
**window-state pattern** (`Box<State>` parked in `GWLP_USERDATA`), the idiom
every stateful example in the rest of the tutorial builds on.

| # | Example | What you learn |
|---|---------|----------------|
| e11 | `e11_message_loop` | What flows through `GetMessageW`/`DispatchMessageW` (console mirror); the `PeekMessageW` alternative |
| e12 | `e12_mouse_clicks` | Button messages; coordinates packed in `lparam`; `GetDC` vs `WM_PAINT`; why naive drawing vanishes |
| e13 | `e13_mouse_move` | **The window-state pattern**; mouse capture; rubber-band drawing with a real data model |
| e14 | `e14_keyboard_input` | `WM_KEYDOWN` (virtual keys) vs `WM_CHAR` (characters) vs `WM_SYSKEYDOWN` (Alt combos) |
| e15 | `e15_keyboard_focus` | Focus, the caret, building a text-entry line by hand — and why EDIT controls exist |
| e16 | `e16_timer_clock` | `SetTimer`/`WM_TIMER` heartbeat; "invalidate on tick, read fresh in paint" |
| e17 | `e17_hit_testing` | `WM_NCHITTEST` — a frameless `WS_POPUP` window you can still drag |
| e18 | `e18_shift_state` | `GetKeyState` vs `GetAsyncKeyState`; modifiers, toggles, Shift+click |
| e19 | `e19_global_hotkey` | `RegisterHotKey` — Ctrl+Alt+H from *any* application |
| e20 | `e20_cursors` | `WM_SETCURSOR`; per-zone cursors (and why the class cursor must be NULL) |

**Suggested order:** straight through. e13's pattern pays off from chapter 3
onward.
