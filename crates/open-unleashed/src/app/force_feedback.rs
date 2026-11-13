//! Force feedback: the original's producers ([`nfs_physics::force_feedback`], read to the
//! instruction, `RE/notes/force-feedback.md`) wired to the race loop, and the one place where their
//! DirectInput effect parameters are **approximated** onto what the rumble backend can drive (a pad's two rumble
//! motors, [`nfs_input::ff`]). The producers' output never reaches the simulation.
//!
//! What is the original's: when the driver runs (every Sc32 tick, before the player's physics
//! step, 0x4121B0 / 0x4123E0), which effect gets which parameters (`FfCommand`), the five `FF.*`
//! levels as the per-effect gains and the total, the jolt's direction / duration / magnitude and its
//! re-arm window, the pause stop (0x412030…).
//!
//! The port's choices are **PORT-DEVIATION D24** (each labelled where it is made):
//! * **A1, the device.** The original arms force feedback only when the player has a
//!   force-feedback-capable DirectInput device bound in the control config (0x4116D0, 0x411D20) and
//!   picks the *last* such device of 32. The port arms it on the **first** connected device SDL3 can
//!   drive effects on (a wheel or another non-pad joystick with an SDL haptic device,
//!   `Gamepads::haptic_device`), whatever the bindings, else on a connected pad that supports rumble
//!   (`SDL_RumbleGamepad` on every desktop OS, the browser's Gamepad API on the web). The haptic
//!   device is a wheel to 0x412D5A's class test
//!   (`devtype & 0xFF00 == 0x600` — `dwDevType` of the legacy `DIDEVICEINSTANCE` (0x56FDB0, filled by `GetDeviceInfo`, returned by 0x532E10 kind 2): subtype 6 = `DIDEVTYPEJOYSTICK_WHEEL`) when SDL types it
//!   `SDL_JOYSTICK_TYPE_WHEEL`; a pad never is (an Xbox pad is GAMEPAD, subtype 4 — `force-feedback.md` §2).
//! * **A2, the effect types — a pad only.** A haptic device gets the four effects one for one
//!   ([`di_call`], `nfs_input::ff::sdl_effect`; units and millisecond rounding there). A pad has
//!   only a strong / weak motor, so its mix is the port's own (`nfs_input::pad_haptics`, revised
//!   2026-11-13): rough ground drives the strong motor, front-wheel scrub the weak one, rear-wheel
//!   scrub / wheelspin the strong one plus chatter on the weak, ABS-like pulses while a front wheel
//!   is locked, and one kick per gear change. The engine's continuous sine is **not** replayed —
//!   at the factory levels it ran the weak motor at 12–20 % all the time and read as a buzz, not
//!   as the car; its job belongs to the engine sound. The spring that carries the grip /
//!   self-aligning feel (`FfCommand::Spring`) has no wheel counterpart on a pad, but its slip signal
//!   (the tyre model's own `Wheel::slip`, the same figure the original's spring reads) is what the
//!   grip cue above is built from, so `FF.RoadGripVolume` does something on a pad. A jolt (a
//!   constant force with a polar direction and an envelope) stays a pulse of **both** motors at
//!   `pad_level` for its duration, at least [`MIN_PULSE_MS`], the direction dropped.
//! * **A3, the clock.** `[0x6559E0]`, the 128 Hz counter the jolt's re-arm window and left/right
//!   parity read, is the simulation's own: `2 · tick`, plus 1 for the collision pass (the effect
//!   only needs a monotonic counter).

use super::*;
use nfs_input::ff::{DiCall, DiSlot, Motor};
use nfs_input::pad_haptics::{PadLevels, PadMixer, PadSignals};
use nfs_physics::force_feedback::{FfCar, FfCommand, FfEffect, FfLevels, ForceFeedback};

/// A2 on a pad: the factory level 50 of both the effect's gain and the total is the effect's own
/// magnitude, capped at the motor's full (PORT-DEVIATION D24).
fn pad_level(magnitude: i32, gain: i32, total: i32) -> f32 {
    ((magnitude as f32 / 10000.0) * (gain as f32 / 50.0) * (total as f32 / 50.0)).min(1.0)
}

/// A2: the shortest pulse a pad gets — a rumble motor needs about this long to spin up, and the
/// landing's jolt is 500 µs (0x41251C).
const MIN_PULSE_MS: u32 = 100;

impl App {
    /// The five `FF.*` accessors, the first dword of the original's five option globals
    /// `[0x657BC8…0x657C48]` (`forcefeedback.lay`, 0…100, default 50).
    pub(crate) fn ff_levels(&self) -> FfLevels {
        let v = |n: &str| self.range_value(n).map_or(50, |r| r.0);
        FfLevels {
            total: v("FF.TotalVolume"),
            road: v("FF.RoadVolume"),
            engine: v("FF.EngineVolume"),
            collision: v("FF.CollisionVolume"),
            grip: v("FF.RoadGripVolume"),
        }
    }

    /// 0x4121B0 / 0x412150, at a race's start: the FF state is made afresh (A1).
    pub(crate) fn ff_arm(&mut self) {
        let _ = self.gamepads.take_ff_hotplug();
        self.gamepads.rumble_stop_all();
        self.pad_ff.reset();
        // A1: a device with SDL haptic effects first (0x411D20 creates the four effects and sets the
        // device gain from `FF.TotalVolume`, 0x5333A0), else a pad's rumble
        if let Some(wheel) = self.gamepads.haptic_arm(self.ff_levels().total) {
            tracing::info!("force feedback: armed — {}", self.gamepads.haptic_describe());
            self.ff = Some(ForceFeedback::new(true, wheel));
            return;
        }
        let enabled = self.gamepads.rumble_supported();
        tracing::info!(levels = ?self.ff_levels(), "force feedback: {} — {}", if enabled { "armed" } else { "off" }, self.gamepads.rumble_describe());
        self.ff = Some(ForceFeedback::new(enabled, false));
    }

    /// 0x412030 / 0x412090 / 0x4120F0, the pause's stop-all.
    pub(crate) fn ff_stop_all(&mut self) {
        if let Some(ff) = self.ff.as_mut() {
            let cmds = ff.stop_all();
            ff_output(&mut self.gamepads, &cmds, self.ff_levels.total);
        }
        // the pad mix's own levels are not in `cmds`; stop them and forget the ABS / gear clock
        self.pad_ff.reset();
        self.gamepads.rumble_stop_all();
    }
}

/// PAD-HANG: whether a force-feedback device connected mid-race re-arms (0x4121B0 runs once, at the
/// race's start): in a race whose force feedback found no device, or one on a pad's rumble when a
/// device with haptic effects (a wheel) has arrived — A1's order. A second pad changes nothing.
pub(crate) fn ff_rearm_wanted(in_race: bool, ff: Option<&ForceFeedback>, haptic_armed: bool, haptic_present: bool) -> bool {
    in_race && ff.is_some_and(|f| !f.enabled || (!haptic_armed && haptic_present))
}

/// What the force-feedback driver reads of the tick besides the car: the five levels, whether a replay is
/// playing, and the game tick (the effects' 128 Hz counter, [`ff_clock`]).
#[derive(Clone, Copy)]
pub(crate) struct FfTick {
    pub(crate) levels: FfLevels,
    pub(crate) replay: bool,
    pub(crate) tick: u64,
}

/// The tick's contact for [`ff_collision`]: the contact word, the body point and the magnitude.
pub(crate) struct FfContact {
    pub(crate) kind: u32,
    pub(crate) body_point: Vec3,
    pub(crate) magnitude: f32,
}

/// The 128 Hz counter the port feeds the effects (A3).
pub(crate) fn ff_clock(tick: u64) -> i32 {
    (tick as i32).wrapping_mul(2)
}

/// The platform call as the haptic backend's [`DiCall`]: the same arguments, one for one.
pub(crate) fn di_call(c: &FfCommand) -> DiCall {
    let slot = |e: FfEffect| match e {
        FfEffect::Collision => DiSlot::Constant,
        FfEffect::Road => DiSlot::Square,
        FfEffect::Engine => DiSlot::Sine,
        FfEffect::Grip => DiSlot::Spring,
    };
    match *c {
        FfCommand::Stop(e) => DiCall::Stop(slot(e)),
        FfCommand::Periodic { effect, magnitude, period_us, gain, .. } => DiCall::Periodic { slot: slot(effect), magnitude, period_us, gain },
        FfCommand::Spring { x, y, gain } => DiCall::Spring { x: (x.offset, x.coefficient), y: (y.offset, y.coefficient), gain },
        FfCommand::Jolt { magnitude, duration_us, direction, attack_level, attack_time, fade_level, fade_time, gain } => {
            DiCall::Constant { magnitude, duration_us, direction, attack_level, attack_time, fade_level, fade_time, gain }
        }
    }
}

/// Send the platform calls to the armed haptic device, or the collision jolts to the pad (A2).
/// The pad's sustained road / grip / ABS levels are the pad mix's own ([`ff_drive`]); the
/// original's periodic road and engine commands are not replayed there, and its engine sine is
/// deliberately dropped (see the module notes).
pub(crate) fn ff_output(pads: &mut nfs_input::gamepad::Gamepads, cmds: &[FfCommand], total: i32) {
    if pads.haptic_armed() {
        for c in cmds {
            pads.haptic_call(&di_call(c));
        }
        return;
    }
    for c in cmds {
        if let FfCommand::Jolt { magnitude, duration_us, gain, .. } = *c {
            pads.rumble_pulse(pad_level(magnitude, gain, total), ((duration_us.max(0) / 1000) as u32).max(MIN_PULSE_MS));
        }
    }
}

/// The driver 0x4123E0, once per Sc32 tick, before the player's physics step (priority 10 against
/// the car's 0x1E): reads the car as the previous tick left it.
pub(crate) fn ff_drive(
    ff: &mut Option<ForceFeedback>,
    pads: &mut nfs_input::gamepad::Gamepads,
    pad_ff: &mut PadMixer,
    t: FfTick,
    car: &FfCar,
    vehicle: &Vehicle,
) {
    let FfTick { levels, replay, tick } = t;
    let Some(ff) = ff.as_mut() else { return };
    if !ff.enabled {
        return;
    }
    let cmds = ff.tick(car, levels, replay, ff_clock(tick));
    ff_output(pads, &cmds, levels.total);
    if pads.haptic_armed() {
        // A1: the race's force feedback went to the wheel; a pad beside it stays quiet
        return;
    }
    if replay {
        // 0x41240E: the effects are not driven during a replay; the pad mix must go quiet too
        pad_ff.reset();
        pads.rumble_stop_all();
        return;
    }
    let out = pad_ff.update(&pad_signals(vehicle), PadLevels { total: levels.total, road: levels.road, grip: levels.grip });
    pads.rumble_sustain(Motor::Strong, out.strong);
    pads.rumble_sustain(Motor::Weak, out.weak);
    if let Some((level, ms)) = out.pulse {
        pads.rumble_pulse(level, ms);
    }
}

/// The car state the pad mix reads. `Wheel::slip` is the tyre model's 0…1 slide figure (what the
/// original's spring reads); the roughness class comes from the wheels actually touching the
/// ground, so two wheels on a shoulder rumble while the body is still over asphalt. Only the
/// 0.1 m classes (grass, gravel, a real kerb) are felt: the 0.03 m classes (2 / 4 / 7 / 10) are
/// mild texture that belongs to the sound, not to a permanent buzz on a straight.
fn pad_signals(v: &Vehicle) -> PadSignals {
    let slip = |i: usize| if v.wheels[i].contact { v.wheels[i].slip } else { 0.0 };
    let roughness = v
        .wheels
        .iter()
        .filter(|w| w.contact)
        .map(|w| {
            let (amp, _) = nfs_physics::vehicle::roughness_profile(w.surface);
            if amp >= 0.08 { (amp / 0.1).clamp(0.0, 1.0) } else { 0.0 }
        })
        .fold(0.0, f32::max);
    PadSignals {
        grounded: v.airborne_count == 0 && v.body_height >= 0.0,
        speed: v.horizontal_speed(),
        throttle: v.throttle.clamp(0.0, 1.0),
        brake: v.brake.clamp(0.0, 1.0),
        front_slip: slip(0).max(slip(1)),
        rear_slip: slip(2).max(slip(3)),
        front_locked: v.wheels[0].locked || v.wheels[1].locked,
        roughness,
        gear: v.gear,
    }
}

/// 0x40F740 → 0x412230, for the player's car, after the tick's contact words are final.
pub(crate) fn ff_collision(ff: &mut Option<ForceFeedback>, pads: &mut nfs_input::gamepad::Gamepads, t: FfTick, car: &FfCar, contact: FfContact) {
    let FfTick { levels, replay, tick } = t;
    let FfContact { kind, body_point, magnitude } = contact;
    let Some(ff) = ff.as_mut() else { return };
    if !ff.enabled {
        return;
    }
    tracing::debug!(kind, magnitude, "force feedback: the player's contact (0x412230)");
    let cmds = ff.collision_jolt(car, nfs_physics::force_feedback::FfImpact { kind: kind, body_point: body_point, magnitude: magnitude }, levels, replay, ff_clock(tick).wrapping_add(1));
    ff_output(pads, &cmds, levels.total);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A2 (PAD-FF): on a pad the factory 50 / 50 is the effect's own magnitude, 100 / 100 is capped at
    /// full, 0 is off.
    #[test]
    fn a_pads_levels_are_relative_to_the_factory_fifty() {
        assert_eq!(pad_level(10000, 50, 50), 1.0);
        assert_eq!(pad_level(5000, 50, 50), 0.5);
        assert_eq!(pad_level(5000, 100, 100), 1.0);
        assert_eq!(pad_level(10000, 25, 50), 0.5);
        assert_eq!(pad_level(10000, 0, 50), 0.0);
    }

    /// PAD-HANG: a pad arriving after the race armed with nothing is armed; one arriving beside the
    /// armed pad is not; a wheel arriving beside an armed pad takes over (A1); outside a race nothing.
    #[test]
    fn a_hot_plugged_device_rearms_force_feedback() {
        let off = ForceFeedback::new(false, false);
        let on = ForceFeedback::new(true, false);
        assert!(ff_rearm_wanted(true, Some(&off), false, false));
        assert!(!ff_rearm_wanted(true, Some(&on), false, false));
        assert!(ff_rearm_wanted(true, Some(&on), false, true));
        assert!(!ff_rearm_wanted(true, Some(&on), true, true));
        assert!(!ff_rearm_wanted(false, Some(&off), false, false));
        assert!(!ff_rearm_wanted(true, None, false, false));
    }
}
