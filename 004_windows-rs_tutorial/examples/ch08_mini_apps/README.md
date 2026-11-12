# Chapter 8 — Mini Apps

Everything so far converges here: twelve complete, self-contained
applications, each ~150–400 lines, each built only from techniques you
already met in e01–e76. The window skeleton stays boring on purpose — the
app logic is the lesson. Read them top to bottom; this is what "knowing
Win32" looks like in the small.

| # | Example | What it is |
|---|---------|------------|
| e77 | `e77_notepad` | Real notepad: menu bar, open/save dialogs, dirty flag, confirm-on-exit |
| e78 | `e78_calculator_basic` | Grid-of-buttons calculator with a two-operand accumulator |
| e79 | `e79_calculator_full` | Chained ops, decimals, %, backspace, full keyboard support, history line |
| e80 | `e80_todo_basic` | The first todo app: add, list, delete |
| e81 | `e81_todo_complete` | Todos with done-state, double-click toggle, status bar counts, filtered views |
| e82 | `e82_todo_persist` | Todos that survive restarts: file persistence + Ctrl+S + a Recent menu |
| e83 | `e83_paint_app` | Doodle pad: strokes model, color/width menus, eraser, save as .bmp |
| e84 | `e84_snake` | The classic: timer-driven grid game, double-buffered, speed ramps |
| e85 | `e85_minesweeper` | 9×9/10 mines, custom-painted board, flood-fill reveal, first-click-safe |
| e86 | `e86_clock_analog` | Analog clock: hand geometry from `GetLocalTime`, smooth double-buffered face |
| e87 | `e87_text_viewer` | Hand-rolled text rendering with scrollbars + a modeless Find dialog with F3 |
| e88 | `e88_unit_converter` | Live as-you-type conversion; data-driven unit table; `EN_CHANGE` in anger |

**Suggested order:** your pick — notepad and the todo ladder (e80→e81→e82)
form the most instructive arc; the games (e84, e85) are the most fun.
