//! The rumble state machine both backends share: two sustained motor levels (strong / weak) and
//! one-shot pulses of both, rate-limited and quantised.
//!
//! The backends differ in how a `(low, high, ms)` call reaches the device: `SDL_RumbleGamepad` on
//! desktop ([`crate::ff`]) and the browser's Gamepad API in the web build
//! ([`crate::gamepad`](crate::gamepad)). Everything above that — which motor a producer drives,
//! when a level is due, how a pulse ends — is the same, so it lives here.
//!
//! **Rate limit (PAD-HANG, PORT-DEVIATION D24).** The producers hand a new level every Sc32 tick
//! (the engine rumble follows the revs, the road rumble the speed), and every `SDL_RumbleGamepad`
//! whose pair differs from the last reaches the driver (`SDL_joystick.c` `SDL_RumbleJoystick`:
//! only an identical pair is "just update the expiration"). On macOS the driver is the
//! GameController one (`SDL_mfijoystick.m` `IOS_JoystickRumble` → `SDL3_RumbleContext
//! rumbleWithLowFrequency:andHighFrequency:`), which sends **both** motors every time — a
//! `CHHapticDynamicParameter` and `-[CHHapticPatternPlayer sendParameters:atTime:]` per motor
//! (`SDL3_RumbleMotor setIntensity:`), plus `stopAtTime:` / `startAtTime:` when a motor goes to /
//! from 0 — i.e. ~128 CoreHaptics messages a second to a Bluetooth pad. So the levels are
//! quantised to [`LEVEL_STEPS`] and a sustained change goes out at most every [`MIN_INTERVAL`];
//! a motor starting or stopping, a pulse and a stop go out at once.

use nfs_platform::time::{Duration, Instant};

/// A sustained rumble: one level per motor, played until it is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motor {
    /// the heavy, low-frequency motor
    Strong,
    /// the light, high-frequency motor
    Weak,
}

/// Where the rumbler's calls go: `SDL_RumbleGamepad` on desktop, the Gamepad API in the browser.
/// Public because [`Rumbler`]'s methods name it, though only the crate's own backends implement it.
pub trait RumbleSink {
    /// `low` / `high` = the strong / weak motors, 0…65535, for `ms`; false when the call was
    /// refused.
    fn rumble(&mut self, low: u16, high: u16, ms: u32) -> bool;
}

/// How long one sustained call runs before the backend stops the motors by itself, on desktop
/// (SDL caps a call at 0xFFFF ms); renewed whenever the caller touches the rumbler after half of
/// it. The browser passes its own, shorter, value to [`Rumbler::new`] because a Gamepad API
/// effect is duration-based and the page has to keep renewing it.
pub const SUSTAIN_MS: u32 = 60_000;
/// The shortest time between two calls that only change a sustained level (PAD-HANG).
pub const MIN_INTERVAL: Duration = Duration::from_millis(100);
/// How many levels a motor has (PAD-HANG): a change smaller than 1/32 of the range is not sent.
pub const LEVEL_STEPS: u16 = 32;
/// How long a refused call (an engine CoreHaptics stopped, a pad that went away) waits before the
/// next try.
const RETRY: Duration = Duration::from_secs(2);

/// The rumble state: both sustained levels and the pulse in flight (on both motors). The backend
/// takes both motors at once for a duration, so every change re-sends the combined state.
pub struct Rumbler {
    strong: f32,
    weak: f32,
    /// (end, level) of a pulse in flight
    pulse: Option<(Instant, f32)>,
    /// (low, high, when) of the last call sent
    sent: Option<(u16, u16, Instant)>,
    /// when a refused call may be tried again
    retry_at: Option<Instant>,
    /// how long one sustained call runs; see [`SUSTAIN_MS`]
    sustain_ms: u32,
}

impl Default for Rumbler {
    fn default() -> Self {
        Self::new(SUSTAIN_MS)
    }
}

/// A 0…1 level as the backend's 0…65535 in [`LEVEL_STEPS`] steps, rounded to the nearest; a level
/// above 0 is at least one step (a quiet rumble does not become a stop).
pub fn quantise(level: f32) -> u16 {
    let l = level.clamp(0.0, 1.0);
    if l <= 0.0 {
        return 0;
    }
    let steps = ((l * f32::from(LEVEL_STEPS)).round() as u16).clamp(1, LEVEL_STEPS);
    (u32::from(steps) * u32::from(u16::MAX) / u32::from(LEVEL_STEPS)) as u16
}

impl Rumbler {
    /// A rumbler whose sustained calls last `sustain_ms` (see [`SUSTAIN_MS`]); the change is
    /// renewed once half of it has passed.
    pub fn new(sustain_ms: u32) -> Self {
        Self { strong: 0.0, weak: 0.0, pulse: None, sent: None, retry_at: None, sustain_ms: sustain_ms.max(2) }
    }

    fn apply(&mut self, pads: &mut impl RumbleSink, urgent: bool) {
        self.apply_at(pads, Instant::now(), urgent);
    }

    /// Send the combined state if it is due. `urgent`: a pulse or a stop the caller asked for.
    fn apply_at(&mut self, pads: &mut impl RumbleSink, now: Instant, urgent: bool) {
        let mut pulse_ended = false;
        let pulse = match self.pulse {
            Some((end, level)) if end > now => level,
            Some(_) => {
                self.pulse = None;
                pulse_ended = true;
                0.0
            }
            None => 0.0,
        };
        // a pulse drives both motors (PAD-FF, D24 A2): over the engine's buzz on the weak motor a
        // strong-only pulse was hardly felt
        let (low, high) = (quantise(self.strong.max(pulse)), quantise(self.weak.max(pulse)));
        let renew = |t: Instant| now.saturating_duration_since(t).as_millis() >= u128::from(self.sustain_ms / 2);
        let due = match self.sent {
            None => low != 0 || high != 0,
            Some((l, h, t)) if l == low && h == high => renew(t),
            Some((l, h, t)) => {
                // a motor starting or stopping, a pulse and its end, a stop: at once; a level change
                // within the rate limit
                let on_off = (l == 0) != (low == 0) || (h == 0) != (high == 0);
                urgent || pulse_ended || on_off || now.saturating_duration_since(t) >= MIN_INTERVAL
            }
        };
        if !due || self.retry_at.is_some_and(|r| now < r && !urgent) {
            return;
        }
        // a lone pulse stops by itself when it ends; with a motor sustained the call runs long
        let ms = match self.pulse {
            Some((end, _)) if self.weak <= 0.0 && self.strong <= 0.0 => end.saturating_duration_since(now).as_millis().max(1) as u32,
            _ => self.sustain_ms,
        };
        tracing::debug!(low, high, ms, urgent, pulse_ended, "rumble call");
        if low == 0 && high == 0 {
            pads.rumble(0, 0, 0);
            self.sent = None;
            self.retry_at = None;
        } else if pads.rumble(low, high, ms) {
            self.sent = Some((low, high, now));
            self.retry_at = None;
        } else {
            tracing::debug!(low, high, "rumble call refused");
            // refused: not again every tick (SDL's GameController driver answers "Haptics engine
            // was stopped" for good once CoreHaptics stopped the engine)
            self.retry_at = Some(now + RETRY);
        }
    }

    /// Send a sustained change the rate limit held back, once its interval is over, and end a pulse
    /// on time; called once a frame (`Gamepads::poll`).
    pub(crate) fn flush(&mut self, pads: &mut impl RumbleSink) {
        self.apply(pads, false);
    }

    /// Set a motor's sustained level, 0…1 (0 stops it).
    pub fn sustain(&mut self, pads: &mut impl RumbleSink, motor: Motor, level: f32) {
        self.set_level(motor, level);
        self.apply(pads, false);
    }

    fn set_level(&mut self, motor: Motor, level: f32) {
        let level = level.clamp(0.0, 1.0);
        match motor {
            Motor::Strong => self.strong = level,
            Motor::Weak => self.weak = level,
        }
    }

    /// A one-shot pulse of both motors at `level` (0…1) for `ms` milliseconds.
    pub fn pulse(&mut self, pads: &mut impl RumbleSink, level: f32, ms: u32) {
        if self.start_pulse(Instant::now(), level, ms) {
            self.apply(pads, true);
        }
    }

    fn start_pulse(&mut self, now: Instant, level: f32, ms: u32) -> bool {
        let level = level.clamp(0.0, 1.0);
        if level <= 0.0 {
            return false;
        }
        self.pulse = Some((now + Duration::from_millis(u64::from(ms.max(1))), level));
        // re-sent even when the pair is the same: the pulse's own duration starts now
        self.sent = None;
        true
    }

    /// Stop both sustained motors (a pause, a menu, the end of the race).
    pub fn stop_all(&mut self, pads: &mut impl RumbleSink) {
        self.strong = 0.0;
        self.weak = 0.0;
        self.pulse = None;
        self.apply(pads, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records every rumble call with the time the rumbler was told.
    #[derive(Default)]
    struct Rec {
        calls: Vec<(u16, u16, u32)>,
        refuse: bool,
    }

    impl RumbleSink for Rec {
        fn rumble(&mut self, low: u16, high: u16, ms: u32) -> bool {
            self.calls.push((low, high, ms));
            !self.refuse
        }
    }

    fn sustain_at(r: &mut Rumbler, s: &mut Rec, now: Instant, motor: Motor, level: f32) {
        r.set_level(motor, level);
        r.apply_at(s, now, false);
    }

    /// PAD-HANG: the engine and road rumble change every Sc32 tick, two producer calls a tick — the
    /// pad gets at most one call per [`MIN_INTERVAL`], not 64 a second.
    #[test]
    fn a_sustained_rumble_is_rate_limited() {
        let (mut r, mut s) = (Rumbler::default(), Rec::default());
        let t0 = Instant::now();
        let ticks = 64; // two seconds of Sc32 ticks
        for i in 0..ticks {
            let now = t0 + Duration::from_micros(31_250 * i);
            sustain_at(&mut r, &mut s, now, Motor::Strong, 0.2 + 0.6 * (i as f32 / ticks as f32));
            sustain_at(&mut r, &mut s, now, Motor::Weak, 0.3 + 0.01 * (i % 7) as f32);
        }
        assert!(s.calls.len() <= 2_000 / MIN_INTERVAL.as_millis() as usize + 1, "{} calls in two seconds", s.calls.len());
        assert!(s.calls.len() >= 5, "the level still follows the producers: {} calls", s.calls.len());
        // the first call went out at once (the motors starting)
        assert_eq!(s.calls[0], (quantise(0.2), 0, SUSTAIN_MS));
    }

    /// A change under one step of [`LEVEL_STEPS`] is never sent; a held-back change goes out with the
    /// first flush after the interval.
    #[test]
    fn small_changes_wait_and_held_back_ones_are_flushed() {
        let (mut r, mut s) = (Rumbler::default(), Rec::default());
        let t0 = Instant::now();
        sustain_at(&mut r, &mut s, t0, Motor::Weak, 0.5);
        assert_eq!(s.calls.len(), 1);
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(500), Motor::Weak, 0.505);
        assert_eq!(s.calls.len(), 1, "under one step");
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(510), Motor::Weak, 0.8);
        assert_eq!(s.calls.len(), 2, "a visible change after the interval goes out");
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(520), Motor::Weak, 0.6);
        assert_eq!(s.calls.len(), 2, "within the interval: held back");
        r.apply_at(&mut s, t0 + Duration::from_millis(560), false);
        assert_eq!(s.calls.len(), 2);
        r.apply_at(&mut s, t0 + Duration::from_millis(615), false);
        assert_eq!(s.calls.last(), Some(&(0, quantise(0.6), SUSTAIN_MS)), "flushed once the interval is over");
    }

    /// A stop, a motor starting and a pulse go out at once, whatever the rate limit.
    #[test]
    fn stops_starts_and_pulses_are_immediate() {
        let (mut r, mut s) = (Rumbler::default(), Rec::default());
        let t0 = Instant::now();
        sustain_at(&mut r, &mut s, t0, Motor::Weak, 0.5);
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(10), Motor::Strong, 0.4);
        assert_eq!(s.calls.last(), Some(&(quantise(0.4), quantise(0.5), SUSTAIN_MS)), "a motor starting");
        assert!(r.start_pulse(t0 + Duration::from_millis(20), 1.0, 300));
        r.apply_at(&mut s, t0 + Duration::from_millis(20), true);
        assert_eq!(s.calls.last(), Some(&(u16::MAX, u16::MAX, SUSTAIN_MS)), "the pulse, on both motors");
        // its end drops both motors back at once
        r.apply_at(&mut s, t0 + Duration::from_millis(330), false);
        assert_eq!(s.calls.last(), Some(&(quantise(0.4), quantise(0.5), SUSTAIN_MS)), "the pulse's end");
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(340), Motor::Strong, 0.0);
        assert_eq!(s.calls.last(), Some(&(0, quantise(0.5), SUSTAIN_MS)), "a motor stopping");
        r.strong = 0.0;
        r.weak = 0.0;
        r.apply_at(&mut s, t0 + Duration::from_millis(345), true);
        assert_eq!(s.calls.last(), Some(&(0, 0, 0)), "the stop");
        let n = s.calls.len();
        r.apply_at(&mut s, t0 + Duration::from_millis(400), false);
        assert_eq!(s.calls.len(), n, "nothing more to send");
        // a lone pulse carries its own duration
        assert!(r.start_pulse(t0 + Duration::from_millis(500), 0.5, 200));
        r.apply_at(&mut s, t0 + Duration::from_millis(500), true);
        assert_eq!(s.calls.last(), Some(&(quantise(0.5), quantise(0.5), 200)));
    }

    /// A refused call is not retried every tick.
    #[test]
    fn a_refused_call_backs_off() {
        let (mut r, mut s) = (Rumbler::default(), Rec { refuse: true, ..Default::default() });
        let t0 = Instant::now();
        for i in 0..64 {
            sustain_at(&mut r, &mut s, t0 + Duration::from_millis(31 * i), Motor::Weak, 0.5);
        }
        assert_eq!(s.calls.len(), 1);
        s.refuse = false;
        sustain_at(&mut r, &mut s, t0 + Duration::from_millis(2_100), Motor::Weak, 0.5);
        assert_eq!(s.calls.len(), 2, "tried again after the back-off");
    }

    /// The browser's sustained call is short (an effect is duration-based), so a held level is
    /// renewed after half of it rather than after [`SUSTAIN_MS`] / 2.
    #[test]
    fn a_shorter_sustain_is_renewed_sooner() {
        let (mut r, mut s) = (Rumbler::new(2_000), Rec::default());
        let t0 = Instant::now();
        sustain_at(&mut r, &mut s, t0, Motor::Strong, 0.5);
        assert_eq!(s.calls.last(), Some(&(quantise(0.5), 0, 2_000)), "the browser's own duration");
        r.apply_at(&mut s, t0 + Duration::from_millis(900), false);
        assert_eq!(s.calls.len(), 1, "not renewed before half of it");
        r.apply_at(&mut s, t0 + Duration::from_millis(1_100), false);
        assert_eq!(s.calls.last(), Some(&(quantise(0.5), 0, 2_000)), "renewed after half of it");
    }

    #[test]
    fn the_levels_are_quantised() {
        assert_eq!(quantise(0.0), 0);
        assert_eq!(quantise(-1.0), 0);
        assert_eq!(quantise(1.0), u16::MAX);
        assert_eq!(quantise(0.001), quantise(1.0 / 32.0), "a quiet rumble stays on");
        assert_eq!(quantise(0.5), quantise(0.51));
    }
}
