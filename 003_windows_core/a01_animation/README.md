# run
- The library wraps the Windows UI Animation (UIAnimationManager2) COM API. Let me check the doc dependency in `lib.rs` and confirm the toolchain before writing `main.rs`.

The library wraps Windows UI Animation (`UIAnimationManager2`) with `Manager`, `Variable`, `Storyboard`/`Keyframe`, `TransitionLibrary`/`Transition`, and re-exports `Result`. The current `main.rs` is just a Hello World stub, so I'll replace it with a console demo that exercises every feature: storyboard keyframe chaining, manager-scheduled transitions, and all three transition types, driven on a 100 ms tick with live ASCII graphs of the interpolated values.

Now build and run it:

The build succeeded but the run failed with `0x800401F0` (`CO_E_NOTINITIALIZED` — "CoInitialize was not called"): `create_instance` uses `CoCreateInstance`, which needs COM initialized on the thread first. Since `lib.rs` is feature-complete, I'll add a small COM init guard directly in `main.rs`:

Done — [main.rs](C:/Users/ytok1/my_projects/window-rs_GUI_training/003_windows_core/a01_animation/src/main.rs) is written, builds clean, and ran successfully end-to-end. It drives the whole library on a shared 100 ms clock and prints live ASCII graphs of the interpolated values.

## What main.rs does

- **COM guard** — the first run failed with `0x800401F0` (`CO_E_NOTINITIALIZED`): `Manager::new()` → `CoCreateInstance` requires COM on the thread. Since `lib.rs` is feature-complete, I added a small RAII `Com` guard in `main.rs` (`CoInitializeEx` STA via `ole32`, `CoUninitialize` on drop) rather than touching the library.
- **Demo 1 — storyboard + keyframe chaining**: `linear(1s, 40)` on a variable starting at 0, then `accelerate_decelerate(1.5s, 100)` attached with `add_transition_at_keyframe` at the keyframe returned by `add_transition`.
- **Demo 2 — manager-scheduled transition**: `Manager::schedule_transition` with `accelerate_decelerate(2s, 0)` as a slide-in 100 → 0, no storyboard involved.
- **Demo 3 — instantaneous transition**: `instantaneous(50)` jumps with no interpolation.

## Run results (verified in the output above)

| Checkpoint | Expected | Actual |
|---|---|---|
| t=1.0s end of linear | 40.00 | `40.00` — perfectly linear (+4.0/tick) |
| t=1.0→2.5s ease segment | smooth S-curve | `44.6 → 50.4 → 57.0 → … → 99.7 → 100.00`, then holds at 100 |
| t=5.0s end of slide-in | 0.00 | `0.00`, decelerating visibly (`1.67 → 0.42 → 0.00`) |
| t=5.1s after instantaneous | 50.00 | exactly `50.00` on the first tick — no ramp |

The keyframe chaining worked exactly as designed: the second transition begins precisely where the first ended (t=1.0s, value 40), with continuous motion and no gap or jump.

One API intentionally not exercised: `Variable::copy_curve` — it needs a real DirectComposition `IDCompositionAnimation` from a DComp device, which isn't available in a console app; it's for when you hand the curve off to DirectComposition-driven rendering.
