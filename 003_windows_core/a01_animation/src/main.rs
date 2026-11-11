//! Console demo for the Windows UI Animation wrapper.
//!
//! Animations are driven on a 100 ms tick: the manager's clock advances,
//! each variable's interpolated value is read back, and a live ASCII graph is
//! printed so the easing curves are visible without a GUI.

use a01_animation::{Manager, Result, TransitionLibrary};
use std::ffi::c_void;
use std::thread::sleep;
use std::time::Duration;
use windows_core::HRESULT;

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pvreserved: *mut c_void, dwcoinit: u32) -> HRESULT;
    fn CoUninitialize();
}

const COINIT_APARTMENTTHREADED: u32 = 0x0;

/// Initializes COM for the thread's lifetime; the animation manager cannot be
/// created until `CoInitializeEx` has been called.
struct Com;

impl Com {
    fn init() -> Result<Self> {
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

const TICK: f64 = 0.1;
const TICK_MS: Duration = Duration::from_millis(100);
const BAR_WIDTH: usize = 50;

/// Renders `value` scaled against `max` as a fixed-width ASCII bar.
fn bar(value: f64, max: f64) -> String {
    let filled = ((value / max).clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;
    format!(
        "[{}{}] {:6.2}",
        "#".repeat(filled),
        ".".repeat(BAR_WIDTH - filled),
        value
    )
}

fn main() -> Result<()> {
    let _com = Com::init()?;
    let manager = Manager::new()?;
    let library = TransitionLibrary::new()?;
    // One clock shared by every demo: the manager's timeline keeps running.
    let mut t = 0.0;

    // Demo 1: a storyboard chains two transitions on one variable. The second
    // starts at the keyframe captured after the first, so the motion is
    // continuous: a linear rise to 40, then an ease in/out to 100.
    println!("Demo 1: storyboard, 0 --linear 1s--> 40 --ease 1.5s--> 100");
    let fade = manager.create_variable(0.0)?;
    let storyboard = manager.create_storyboard()?;
    let after_rise = storyboard.add_transition(&fade, &library.linear(1.0, 40.0)?)?;
    storyboard.add_transition_at_keyframe(
        &fade,
        &library.accelerate_decelerate(1.5, 100.0, 0.3, 0.6)?,
        after_rise,
    )?;
    storyboard.schedule(t)?;

    for i in 0..=30 {
        t = i as f64 * TICK;
        manager.update(t)?;
        println!("  t={t:>4.1}s  {}", bar(fade.value()?, 100.0));
        sleep(TICK_MS);
    }
    println!("  final: fade = {:.3}\n", fade.value()?);

    // Demo 2: the manager schedules a single transition directly, no
    // storyboard needed. A window sliding in from the right: 100 -> 0.
    println!("Demo 2: manager transition, slide-in 100 --ease 2s--> 0");
    let position = manager.create_variable(100.0)?;
    let slide = library.accelerate_decelerate(2.0, 0.0, 0.4, 0.6)?;
    manager.schedule_transition(&position, &slide, t)?;

    let start = t;
    for i in 0..=20 {
        t = start + i as f64 * TICK;
        manager.update(t)?;
        println!("  t={t:>4.1}s  {}", bar(position.value()?, 100.0));
        sleep(TICK_MS);
    }
    println!("  final: position = {:.3}\n", position.value()?);

    // Demo 3: an instantaneous transition jumps to its final value without
    // interpolating, so one tick later the variable has already landed.
    println!("Demo 3: instantaneous jump, position 0 -> 50");
    manager.schedule_transition(&position, &library.instantaneous(50.0)?, t)?;
    t += TICK;
    manager.update(t)?;
    println!("  t={t:>4.1}s  {}", bar(position.value()?, 100.0));

    println!("\nAll demos completed.");
    Ok(())
}
