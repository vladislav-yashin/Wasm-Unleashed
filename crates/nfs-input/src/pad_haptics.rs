//! The port's own gamepad haptics mix (PORT-DEVIATION D24, A2, revised): the car's state mapped
//! onto a pad's two rumble motors.
//!
//! On a wheel the original's four DirectInput effects are left alone (the SDL haptic backend);
//! a pad has only a strong (low-frequency) and a weak (high-frequency) motor, so the original's
//! effects cannot be replayed one for one. The first port mapping took the two periodic effects
//! literally: the engine's sine ran the weak motor all the time and the road's square ran the
//! strong one on rough ground. That is a permanent buzz which says little about the car.
//!
//! This mix instead follows what a pad can express and what a driver listens for:
//!
//! | signal | motors | effect |
//! |---|---|---|
//! | rough ground (grass, gravel, real kerbs, shoulders) | strong | road texture, none on asphalt or the mild 0.03 m classes |
//! | front wheels sliding (understeer) | weak | tyre scrub, silent below [`FRONT_SLIP_START`] |
//! | rear wheels sliding (oversteer, wheelspin) | strong | low rumble, plus weak chatter under power |
//! | front wheels locked while braking | weak | ABS-like 8 Hz pulses |
//! | a gear change | both | one short kick |
//! | impact | both | the original's jolt, sent by the caller, not here |
//!
//! The engine's continuous sine is **not** replayed: its job is done by the engine sound. The
//! grip signal is fed from the tyre model (`Vehicle::wheels[i].slip`, the same slip the original's
//! spring reads), so normal cornering is silent and the motors only speak when a tyre is at or
//! past its limit. Wheel slip is updated by the physics step before the force-feedback driver of
//! the *next* tick reads it, exactly as the original's 0x4123E0 reads the previous tick's car.

/// The car's state the mixer reads, taken from the physics body by the caller once a 32 Hz tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadSignals {
    /// The car is on its wheels and not airborne; road, grip and ABS are silenced otherwise.
    pub grounded: bool,
    /// Horizontal speed, m/s.
    pub speed: f32,
    /// Effective throttle, 0…1.
    pub throttle: f32,
    /// Effective brake, 0…1.
    pub brake: f32,
    /// The larger of the two front wheels' slip, 0…1 (the tyre model's `Wheel::slip`).
    pub front_slip: f32,
    /// The larger of the two rear wheels' slip, 0…1.
    pub rear_slip: f32,
    /// A front wheel is locked while braking (`Wheel::locked`).
    pub front_locked: bool,
    /// The roughest contacting surface, 0…1: 1 for grass / gravel / kerbs of the 0.1 m class, 0.3
    /// for the 0.03 m class, 0 on asphalt and in the air.
    pub roughness: f32,
    /// The current gear slot: 0 reverse, 1 neutral, 2… forward.
    pub gear: usize,
}

/// The three `FF.*` levels the pad mix uses, 0…100 each (the accessors' range).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PadLevels {
    /// `FF.TotalVolume` — the global gain and the gear-change kick's amplitude.
    pub total: i32,
    /// `FF.RoadVolume` — the rough-ground texture.
    pub road: i32,
    /// `FF.RoadGripVolume` — tyre scrub, ABS and wheelspin.
    pub grip: i32,
}

/// What one tick asks the pad's motors to do. `strong` / `weak` are sustained levels, 0…1; `pulse`
/// is a one-shot the caller sends through the rumble backend's pulse path (an impact takes
/// precedence over it when both happen on the same tick).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadOutput {
    pub strong: f32,
    pub weak: f32,
    pub pulse: Option<(f32, u32)>,
}

/// The front slip at which tyre scrub starts. `Wheel::slip` is 0 until the friction circle is
/// exceeded, so normal cornering never reaches this; the front slip figure rises slowly (the
/// rear's is weighted twelve to the front's half in the tyre model, whose `slip` is `0.5·excess`
/// at the front and `12·excess` at the rear), hence the lower band.
pub const FRONT_SLIP_START: f32 = 0.04;
/// The front slip at which understeer scrub reaches its full level.
pub const FRONT_SLIP_FULL: f32 = 0.35;
/// The rear slip at which oversteer / wheelspin rumble starts: high enough that the mild
/// over-limit a powerful car shows under part throttle stays quiet.
pub const REAR_SLIP_START: f32 = 0.4;
/// The rear slip at which the rear cue reaches its full level.
pub const REAR_SLIP_FULL: f32 = 1.0;
/// Ticks the rear slip must stay above [`REAR_SLIP_START`] before the rumble starts: a one-tick
/// spike (a gear change, a bump) is not wheelspin.
const REAR_HOLD: u8 = 3;
/// The weak motor's level at full front-wheel scrub (the factory `FF.RoadGripVolume` / total 50).
pub const FRONT_SCRUB_LEVEL: f32 = 0.25;
/// The strong motor's level at full rear-wheel slide (the factory levels).
pub const REAR_SCRUB_LEVEL: f32 = 0.27;
/// The weak motor's extra level at full wheelspin under power.
pub const WHEELSPIN_LEVEL: f32 = 0.17;
/// The weak motor's pulse level at a locked front wheel.
pub const ABS_LEVEL: f32 = 0.28;
/// How fast a rising level is followed per 32 Hz tick (≈45 ms time constant).
const ATTACK: f32 = 0.45;
/// How fast a level falls back per tick (≈150 ms), so a short slide does not linger on a straight.
const RELEASE: f32 = 0.2;
/// Outputs below this are snapped to zero: the rumble backend quantises to 1/32, and a 3 % tail
/// would otherwise buzz faintly after a cue has gone.
const SILENCE: f32 = 0.02;
/// The ABS chatter's period and pulse length in 32 Hz ticks (4 ticks = 125 ms, 8 Hz).
const ABS_PERIOD: u64 = 4;
const ABS_PULSE_MS: u32 = 50;
/// The gear-change kick's length and level relative to `FF.TotalVolume`.
const SHIFT_PULSE_MS: u32 = 60;
const SHIFT_LEVEL: f32 = 0.14;

/// The state behind [`PadMixer::update`]: the smoothed levels, the ABS clock and the last gear.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadMixer {
    strong: f32,
    weak: f32,
    tick: u64,
    last_gear: Option<usize>,
    /// consecutive ticks the rear slip has been above [`REAR_SLIP_START`]
    rear_hold: u8,
}

impl PadMixer {
    /// Forget everything: the next tick starts silent and does not fire a gear kick or an ABS
    /// pulse for the past. Called when force feedback is armed or stopped (a race's start, a
    /// pause, the race's end).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// One 32 Hz tick. Returns the sustained levels and, for an ABS or gear-change tick, a pulse.
    pub fn update(&mut self, signals: &PadSignals, levels: PadLevels) -> PadOutput {
        let total_gain = gain(levels.total, levels.total);
        let road_gain = gain(levels.road, levels.total);
        let grip_gain = gain(levels.grip, levels.total);

        // a gear change: one edge, forward gears only, and only once per change
        let mut pulse = None;
        if self.last_gear.is_some_and(|last| last != signals.gear && last >= 2 && signals.gear >= 2 && signals.speed > 1.0) {
            let level = SHIFT_LEVEL * total_gain;
            if level > 0.0 {
                pulse = Some((level, SHIFT_PULSE_MS));
            }
        }
        self.last_gear = Some(signals.gear);

        if signals.grounded && signals.rear_slip >= REAR_SLIP_START {
            self.rear_hold = self.rear_hold.saturating_add(1);
        } else {
            self.rear_hold = 0;
        }
        let (strong_target, weak_target) = if signals.grounded {
            let speed = (signals.speed / 20.0).clamp(0.0, 1.0);
            // asphalt and the mild 0.03 m classes are zero in `pad_signals`, so a smooth road is
            // silent; only the 0.1 m class (grass, gravel, a real kerb) rumbles
            let road = signals.roughness.clamp(0.0, 1.0) * road_gain * 0.28 * speed;
            let front = slip_level(signals.front_slip, FRONT_SLIP_START, FRONT_SLIP_FULL) * grip_gain * FRONT_SCRUB_LEVEL;
            let rear_level = if self.rear_hold >= REAR_HOLD { slip_level(signals.rear_slip, REAR_SLIP_START, REAR_SLIP_FULL) } else { 0.0 };
            let rear = rear_level * grip_gain * REAR_SCRUB_LEVEL;
            let spin = if signals.throttle > 0.4 { rear_level * grip_gain * WHEELSPIN_LEVEL } else { 0.0 };
            (road.max(rear), front.max(spin))
        } else {
            (0.0, 0.0)
        };
        self.strong = approach(self.strong, strong_target);
        self.weak = approach(self.weak, weak_target);

        // ABS: the tyre model marks a locked front wheel under braking. Pulse the weak motor at
        // roughly the rate an ABS pump would, using the grip level as its amplitude.
        if signals.grounded && signals.front_locked && signals.brake > 0.1 && signals.speed > 2.0 && self.tick % ABS_PERIOD == 0 {
            let level = grip_gain * ABS_LEVEL;
            if level > 0.0 {
                pulse = Some(match pulse {
                    Some((l, ms)) if l > level => (l, ms),
                    _ => (level, ABS_PULSE_MS),
                });
            }
        }
        self.tick = self.tick.wrapping_add(1);
        PadOutput { strong: self.strong, weak: self.weak, pulse }
    }
}

/// A 0…1 level from its `FF.*` volume and the global total, relative to the factory 50 / 50 (the
/// same convention as the original effects' gains): 50 / 50 is 1, 25 / 50 half, 100 / 100 still 1.
fn gain(volume: i32, total: i32) -> f32 {
    ((volume as f32 / 50.0) * (total as f32 / 50.0)).clamp(0.0, 1.0)
}

/// The cue for a wheel at `slip`: silent below `start`, eased in to `full`.
fn slip_level(slip: f32, start: f32, full: f32) -> f32 {
    let x = ((slip - start) / (full - start)).clamp(0.0, 1.0);
    x * x
}

fn approach(current: f32, target: f32) -> f32 {
    let next = if target > current { current + (target - current) * ATTACK } else { current + (target - current) * RELEASE };
    if next < SILENCE { 0.0 } else { next }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: PadLevels = PadLevels { total: 50, road: 50, grip: 50 };

    fn car() -> PadSignals {
        PadSignals { grounded: true, speed: 25.0, throttle: 0.0, brake: 0.0, front_slip: 0.0, rear_slip: 0.0, front_locked: false, roughness: 0.0, gear: 3 }
    }

    fn settle(mixer: &mut PadMixer, signals: &PadSignals) -> PadOutput {
        let mut out = PadOutput::default();
        for _ in 0..20 {
            out = mixer.update(signals, LEVELS);
        }
        out
    }

    /// A smooth track at speed and a car within its grip limits: nothing buzzes.
    #[test]
    fn asphalt_and_grip_are_silent() {
        let mut m = PadMixer::default();
        let out = settle(&mut m, &car());
        assert_eq!((out.strong, out.weak), (0.0, 0.0));
        assert!(out.pulse.is_none());
    }

    /// A shoulder or a grass verge rumbles the strong motor; asphalt does not.
    #[test]
    fn rough_ground_drives_the_strong_motor() {
        let mut m = PadMixer::default();
        let mut rough = car();
        rough.roughness = 1.0;
        let out = settle(&mut m, &rough);
        assert!(out.strong > 0.2, "strong {}", out.strong);
        assert_eq!(out.weak, 0.0);

        let mut smooth = PadMixer::default();
        let out = settle(&mut smooth, &car());
        assert_eq!(out.strong, 0.0, "asphalt stays quiet");
    }

    /// Front slip is the weak motor's tyre scrub and starts only past [`FRONT_SLIP_START`]; rear
    /// slip is the strong motor's oversteer rumble.
    #[test]
    fn slip_maps_to_the_motors_with_a_dead_zone() {
        let mut m = PadMixer::default();
        let mut front = car();
        front.front_slip = FRONT_SLIP_START * 0.5;
        assert_eq!(settle(&mut m, &front).weak, 0.0, "below the dead zone");

        front.front_slip = FRONT_SLIP_FULL;
        assert!(settle(&mut m, &front).weak > 0.2, "front scrub");

        let mut rear = car();
        rear.rear_slip = REAR_SLIP_FULL;
        let mut m = PadMixer::default();
        let out = settle(&mut m, &rear);
        assert!(out.strong > 0.2, "rear rumble {}", out.strong);
        assert_eq!(out.weak, 0.0, "rear slip alone does not scrub the weak motor");
    }

    /// A single-tick rear slip spike (a gear change, a bump) and the mild over-limit of part
    /// throttle do not rumble; a real, sustained wheelspin does.
    #[test]
    fn a_brief_or_mild_rear_slip_does_not_rumble() {
        let mut m = PadMixer::default();
        let mut brief = car();
        brief.rear_slip = 1.0;
        // one tick above the threshold, then back to grip
        assert_eq!(m.update(&brief, LEVELS).strong, 0.0, "held back");
        let mut grip = car();
        grip.rear_slip = 0.2;
        let out = settle(&mut m, &grip);
        assert_eq!(out.strong, 0.0, "mild rear slip stays quiet");

        let mut sustained = car();
        sustained.rear_slip = 1.0;
        let out = settle(&mut m, &sustained);
        assert!(out.strong > 0.2, "a real wheelspin rumbles: {}", out.strong);
    }

    /// The full-slip cues stop at the tuned maxima; they do not creep up with the slip past
    /// `full` or with a second source.
    #[test]
    fn the_grip_cues_keep_their_maxima() {
        let mut front = PadMixer::default();
        let mut s = car();
        s.front_slip = FRONT_SLIP_FULL;
        let weak = settle(&mut front, &s).weak;
        assert!(weak > 0.0 && weak <= FRONT_SCRUB_LEVEL + 1e-5, "front {weak}");

        let mut rear = PadMixer::default();
        let mut s = car();
        s.rear_slip = REAR_SLIP_FULL;
        let strong = settle(&mut rear, &s).strong;
        assert!(strong > 0.0 && strong <= REAR_SCRUB_LEVEL + 1e-5, "rear {strong}");

        let mut spin = PadMixer::default();
        let mut s = car();
        s.rear_slip = REAR_SLIP_FULL;
        s.throttle = 1.0;
        let out = settle(&mut spin, &s);
        assert!(out.weak > 0.0 && out.weak <= WHEELSPIN_LEVEL + 1e-5, "spin {}", out.weak);
    }

    /// Power over a spinning rear axle adds high-frequency chatter on top of the rear rumble.
    #[test]
    fn wheelspin_adds_weak_chatter() {
        let mut m = PadMixer::default();
        let mut spin = car();
        spin.rear_slip = REAR_SLIP_FULL;
        spin.throttle = 0.9;
        let out = settle(&mut m, &spin);
        assert!(out.strong > 0.2 && out.weak > 0.1, "strong {} weak {}", out.strong, out.weak);
    }

    /// Airborne: no road and no grip, even when the wheels' last slip is stale.
    #[test]
    fn airborne_is_silent() {
        let mut m = PadMixer::default();
        let mut air = car();
        air.grounded = false;
        air.roughness = 1.0;
        air.front_slip = 0.6;
        air.rear_slip = 0.6;
        let out = settle(&mut m, &air);
        assert_eq!((out.strong, out.weak), (0.0, 0.0));
    }

    /// The volumes scale the cue; all zero is silence.
    #[test]
    fn the_ff_levels_scale_the_cue() {
        let mut m = PadMixer::default();
        let mut rough = car();
        rough.roughness = 1.0;
        let quiet = PadLevels { total: 50, road: 0, grip: 50 };
        let mut out = PadOutput::default();
        for _ in 0..20 {
            out = m.update(&rough, quiet);
        }
        assert_eq!(out.strong, 0.0, "FF.RoadVolume 0");

        let off = PadLevels { total: 0, road: 50, grip: 50 };
        for _ in 0..20 {
            out = m.update(&rough, off);
        }
        assert_eq!((out.strong, out.weak), (0.0, 0.0), "FF.TotalVolume 0");
    }

    /// A locked front wheel under braking pulses the weak motor about every 125 ms; no lock, no
    /// pulse.
    #[test]
    fn a_locked_front_wheel_pulses_like_abs() {
        let mut m = PadMixer::default();
        let mut locked = car();
        locked.front_locked = true;
        locked.brake = 1.0;
        let mut pulses = 0;
        let mut loudest = 0.0f32;
        for _ in 0..20 {
            if let Some((level, _)) = m.update(&locked, LEVELS).pulse {
                pulses += 1;
                loudest = loudest.max(level);
            }
        }
        assert_eq!(pulses, 5, "one pulse every four ticks");
        assert!(loudest > 0.0 && loudest <= ABS_LEVEL + 1e-5, "ABS level {loudest}");

        let mut rolling = car();
        rolling.brake = 1.0;
        let mut m = PadMixer::default();
        for _ in 0..20 {
            assert!(m.update(&rolling, LEVELS).pulse.is_none(), "no lock, no ABS");
        }
    }

    /// A forward gear change is one short kick, not one per tick; neutral / reverse do not kick,
    /// and the first tick after [`PadMixer::reset`] never kicks for the gear it starts in.
    #[test]
    fn a_gear_change_kicks_once() {
        let mut m = PadMixer::default();
        let mut s = car();
        assert!(m.update(&s, LEVELS).pulse.is_none(), "the starting gear");
        s.gear = 4;
        let first = m.update(&s, LEVELS);
        assert!(first.pulse.is_some_and(|(level, ms)| level > 0.0 && ms == SHIFT_PULSE_MS), "the upshift");
        for _ in 0..10 {
            assert!(m.update(&s, LEVELS).pulse.is_none(), "the same gear again");
        }
        // reverse / neutral are not a racing shift
        s.gear = 1;
        assert!(m.update(&s, LEVELS).pulse.is_none(), "into neutral");
        s.gear = 2;
        assert!(m.update(&s, LEVELS).pulse.is_none(), "out of neutral");
    }

    /// A cue falls away smoothly instead of stepping to zero.
    #[test]
    fn a_released_cue_decays() {
        let mut m = PadMixer::default();
        let mut rough = car();
        rough.roughness = 1.0;
        let held = settle(&mut m, &rough).strong;
        rough.roughness = 0.0;
        let first = m.update(&rough, LEVELS).strong;
        assert!(first > 0.0 && first < held, "held {held}, after one tick {first}");
        assert!(settle(&mut m, &rough).strong < 0.05, "gone after twenty ticks");
    }
}
