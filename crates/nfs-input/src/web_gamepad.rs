use crate::rumbler::Rumbler;
use crate::{InputState, bindings::PadRecord};
use wasm_bindgen::JsCast;

/// The browser's haptics: `Gamepad.vibrationActuator.playEffect("dual-rumble", …)` in Chrome,
/// Edge and Safari 16.4+, or the legacy `hapticActuators[0].pulse` (kept for browsers that still
/// implement it; Firefox exposes the property but has no backend behind it, Bugzilla 680289).
/// `web-sys` exposes the vibration actuator only under `web_sys_unstable_apis` (and
/// `hapticActuators` not at all in current Chrome), so the call is made from a small JS shim. Both
/// entry points catch the promise's rejection: a hidden page, a controller without haptics or a
/// closed actuator answers `InvalidStateError` / `NotSupportedError`, and an unhandled rejection
/// would reach the console.
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
function nfsRumbleTargets() {
  if (typeof navigator.getGamepads !== 'function') return [];
  let pads;
  try { pads = navigator.getGamepads(); } catch (_) { return []; }
  const out = [];
  for (const pad of pads) {
    if (!pad || !pad.connected || pad.mapping !== 'standard') continue;
    const actuator = pad.vibrationActuator;
    if (actuator && typeof actuator.playEffect === 'function') out.push({ actuator, pulse: false });
    else if (pad.hapticActuators && pad.hapticActuators[0] && typeof pad.hapticActuators[0].pulse === 'function') out.push({ actuator: pad.hapticActuators[0], pulse: true });
  }
  return out;
}
function nfsRumbleIgnore(result) {
  if (result && typeof result.catch === 'function') result.catch(() => {});
  return result;
}
export function nfsRumbleAvailable() {
  return nfsRumbleTargets().length > 0;
}
export function nfsRumble(strong, weak, ms) {
  const target = nfsRumbleTargets()[0];
  if (!target) return false;
  const low = Math.max(0, Math.min(1, strong / 65535));
  const high = Math.max(0, Math.min(1, weak / 65535));
  try {
    if (target.pulse) {
      if (low === 0 && high === 0) return true;
      nfsRumbleIgnore(target.actuator.pulse(Math.max(low, high), Math.max(1, ms | 0)));
      return true;
    }
    if (low === 0 && high === 0) {
      if (typeof target.actuator.reset === 'function') nfsRumbleIgnore(target.actuator.reset());
      else nfsRumbleIgnore(target.actuator.playEffect('dual-rumble', { duration: 0, strongMagnitude: 0, weakMagnitude: 0 }));
      return true;
    }
    nfsRumbleIgnore(target.actuator.playEffect('dual-rumble', {
      startDelay: 0,
      duration: Math.max(1, ms | 0),
      strongMagnitude: low,
      weakMagnitude: high,
    }));
    return true;
  } catch (_) {
    return false;
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = nfsRumbleAvailable)]
    fn rumble_available() -> bool;
    #[wasm_bindgen(js_name = nfsRumble)]
    fn rumble_play(strong: u16, weak: u16, ms: u32) -> bool;
}

/// Where the browser rumbler's calls go ([`crate::rumbler::RumbleSink`]).
struct WebSink;

impl crate::rumbler::RumbleSink for WebSink {
    fn rumble(&mut self, low: u16, high: u16, ms: u32) -> bool {
        rumble_play(low, high, ms)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Button {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    RightTrigger,
    LeftTrigger2,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhotoPad {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub trigger_left: f32,
    pub trigger_right: f32,
    /// A, B, X, Y (south, east, west, north), the shoulders, the d-pad, the left stick's click and Start
    pub south: bool,
    pub east: bool,
    pub west: bool,
    pub north: bool,
    pub bumper_left: bool,
    pub bumper_right: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub left_thumb: bool,
    pub start: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadAxisSet {
    /// bit `n` set when the pad has axis `n`, in `DIJOYSTATE` order
    pub mask: u8,
    /// each axis's current value, scaled to the 0…0x80 the dead-zone accessors use
    pub values: [i32; 8],
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadSnapshot {
    /// left stick X / Y, right stick X / Y (SDL: Y grows downward, as DirectInput's)
    pub sticks: [i16; 4],
    /// left / right trigger, 0…32767
    pub triggers: [i16; 2],
    /// d-pad up / right / down / left
    pub dpad: [bool; 4],
    /// the 15 buttons in [`BUTTON_ORDER`] (the entries of `C`, `Z` and the two trigger slots are
    /// ignored by [`pad_record`])
    pub buttons: [bool; 15],
}
pub fn analog_read(v: i32, lo: i32, hi: i32) -> i32 {
    let (l, h) = (lo << 8, hi << 8);
    if l < h {
        if v < l {
            0
        } else if v > h {
            255
        } else {
            (v - l) * 255 / (h - l)
        }
    } else if l > h {
        if v > l {
            0
        } else if v < h {
            255
        } else {
            (l - v) * 255 / (l - h)
        }
    } else {
        0
    }
}
pub fn axis_band(min: i32, max: i32, dir: i32) -> (i32, i32) {
    ((min * dir + 0x80).max(0), (max * dir + 0x80).min(0xFF))
}
pub fn stick_steer(x: f32, min: i32, max: i32) -> i32 {
    let v = (((x.clamp(-1.0, 1.0) + 1.0) * 0.5) * 65535.0).round() as i32;
    let (rl, rh) = axis_band(min, max, 1);
    let (ll, lh) = axis_band(min, max, -1);
    (analog_read(v, rl, rh) - analog_read(v, ll, lh)) / 4
}
pub fn photo_deadzone(v: f32) -> f32 {
    const DZ: f32 = 0.15;
    if v.abs() <= DZ { 0.0 } else { (v - DZ * v.signum()) / (1.0 - DZ) }
}
fn di_value(v: i16) -> i32 {
    i32::from(v) + 32768
}
fn trigger_value(v: i16) -> i32 {
    0x8000 + i32::from(v.max(0))
}
pub fn pad_record(p: &PadSnapshot) -> PadRecord {
    let mut r = PadRecord { mask: 0x3F, standard: true, ..Default::default() };
    for (slot, v) in [0usize, 1, 3, 4].into_iter().zip(p.sticks) {
        r.axes[slot] = di_value(v);
    }
    r.axes[2] = trigger_value(p.triggers[0]);
    r.axes[5] = trigger_value(p.triggers[1]);
    for (n, b) in BUTTON_ORDER.iter().enumerate() {
        if !matches!(b, Button::C | Button::Z | Button::LeftTrigger2 | Button::RightTrigger2) {
            r.buttons[n] = p.buttons[n];
            r.button_mask |= 1 << n;
        }
    }
    r.pov[0] = hat_of(p.dpad[0], p.dpad[1], p.dpad[2], p.dpad[3]);
    r
}
fn hat_of(up: bool, right: bool, down: bool, left: bool) -> i32 {
    HAT[up as usize | (right as usize) << 1 | (down as usize) << 2 | (left as usize) << 3]
}
pub const BUTTON_ORDER: [Button; 15] = [
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::C,
    Button::Z,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
];
const HAT: [i32; 16] = [-1, 0, 9000, 4500, 18000, -1, 13500, -1, 27000, 31500, -1, -1, 22500, -1, -1, -1];
impl PadSnapshot {
    /// The port's button `b` ([`BUTTON_ORDER`]).
    pub fn button(&self, b: Button) -> bool {
        BUTTON_ORDER.iter().position(|x| *x == b).is_some_and(|n| self.buttons[n])
    }
}
/// How long one browser rumble effect runs. A Gamepad API effect is duration-based (there is no
/// "infinite" one), so [`Rumbler`] renews a sustained level after half of it; the value is also
/// longer than the longest jolt (1.5 s, the platform's clamp), so a pulse never ends early.
const WEB_SUSTAIN_MS: u32 = 2_000;

pub struct Gamepads {
    pads: Vec<(String, PadSnapshot)>,
    previous: Vec<PadSnapshot>,
    hotplug: bool,
    /// set when a controller that can rumble was connected; taken by [`Gamepads::take_ff_hotplug`]
    ff_hotplug: bool,
    rumbler: Rumbler,
}

impl Default for Gamepads {
    fn default() -> Self {
        Self { pads: Vec::new(), previous: Vec::new(), hotplug: false, ff_hotplug: false, rumbler: Rumbler::new(WEB_SUSTAIN_MS) }
    }
}

impl Gamepads {
    pub fn new() -> Self { Self::default() }
    pub fn poll(&mut self, input: &mut InputState) {
        let Some(window) = web_sys::window() else { return };
        let Ok(devices) = window.navigator().get_gamepads() else { return };
        self.previous = self.pads.iter().map(|(_, p)| *p).collect();
        self.pads.clear();
        for value in devices.iter() {
            let Ok(pad) = value.dyn_into::<web_sys::Gamepad>() else { continue };
            if !pad.connected() || pad.mapping() != web_sys::GamepadMappingType::Standard { continue; }
            let axes = pad.axes(); let buttons = pad.buttons();
            let axis = |i| axes.get(i).as_f64().unwrap_or(0.0).clamp(-1.0, 1.0);
            let button = |i| buttons.get(i).dyn_into::<web_sys::GamepadButton>().ok();
            let pressed = |i| button(i).is_some_and(|b| b.pressed());
            let mut snapshot = PadSnapshot::default();
            snapshot.sticks = [0, 1, 2, 3].map(|i| (axis(i) * 32767.0) as i16);
            snapshot.triggers = [6, 7].map(|i| (button(i).map_or(0.0, |b| b.value()).clamp(0.0, 1.0) * 32767.0) as i16);
            snapshot.dpad = [12, 15, 13, 14].map(pressed);
            for (slot, browser) in [(0,0),(1,1),(2,3),(3,2),(6,4),(7,5),(10,8),(11,9),(12,16),(13,10),(14,11)] { snapshot.buttons[slot] = pressed(browser); }
            let prior = self.previous.get(self.pads.len()).copied().unwrap_or_default();
            for (index, nav) in [(0,crate::MenuNav::Up),(1,crate::MenuNav::Right),(2,crate::MenuNav::Down),(3,crate::MenuNav::Left)] {
                if snapshot.dpad[index] && !prior.dpad[index] { input.pad_menu.push(nav); }
            }
            for (index, nav) in [(0,crate::MenuNav::Select),(1,crate::MenuNav::Back),(2,crate::MenuNav::Alt),(3,crate::MenuNav::Aux)] {
                if snapshot.buttons[index] && !prior.buttons[index] { input.pad_menu.push(nav); }
            }
            for (axis, negative, positive) in [(0,crate::MenuNav::Left,crate::MenuNav::Right),(1,crate::MenuNav::Up,crate::MenuNav::Down)] {
                let direction = |value: i16| if value < -19660 { -1 } else if value > 19660 { 1 } else { 0 };
                let next = direction(snapshot.sticks[axis]);
                if next != 0 && next != direction(prior.sticks[axis]) { input.pad_menu.push(if next < 0 { negative } else { positive }); }
            }
            self.pads.push((pad.id(), snapshot));
        }
        self.hotplug |= self.pads.len() != self.previous.len();
        // PAD-HANG: force feedback is armed once, at the race's start, so a controller that
        // appears later (or that the browser only reveals after a button press) re-arms it
        if self.pads.len() > self.previous.len() && rumble_available() {
            self.ff_hotplug = true;
        }
        // a sustained level the rate limit held back, or the end of a pulse, goes out now
        self.rumbler.flush(&mut WebSink);
        input.set_pads(&self.records());
    }
    pub fn records(&self) -> Vec<PadRecord> { self.pads.iter().map(|(_, p)| pad_record(p)).collect() }
    pub fn records_into(&self, out: &mut Vec<PadRecord>) { *out = self.records(); }
    pub fn devices(&self) -> Vec<(String, bool)> { self.pads.iter().map(|(n,_)| (n.clone(),true)).collect() }
    pub fn axis_sets(&self) -> Vec<PadAxisSet> { self.records().iter().map(|r| PadAxisSet { mask:r.mask, values:r.axes }).collect() }
    pub fn pad_snapshot(&self) -> Option<PadSnapshot> { self.pads.first().map(|(_,p)| *p) }
    pub fn take_pad_hotplug(&mut self) -> bool { std::mem::take(&mut self.hotplug) }
    pub fn take_ff_hotplug(&mut self) -> bool { std::mem::take(&mut self.ff_hotplug) }
    /// Whether a connected standard controller exposes haptics the browser can drive.
    pub fn rumble_supported(&self) -> bool { rumble_available() }
    pub fn rumble_sustain(&mut self, motor: crate::ff::Motor, level: f32) {
        self.rumbler.sustain(&mut WebSink, motor, level);
    }
    pub fn rumble_pulse(&mut self, level: f32, ms: u32) {
        self.rumbler.pulse(&mut WebSink, level, ms);
    }
    pub fn rumble_stop_all(&mut self) {
        self.rumbler.stop_all(&mut WebSink);
    }
    /// One line for the log: the browser backend and how many standard pads are connected. The
    /// shim drives the first of them that exposes a haptic actuator.
    pub fn rumble_describe(&self) -> String {
        match self.pads.len() {
            0 => "Gamepad API rumble, no controller connected".to_string(),
            _ if rumble_available() => format!("Gamepad API dual-rumble (strong / weak motor), {} standard controller(s)", self.pads.len()),
            _ => "Gamepad API rumble, the connected controller exposes no haptics".to_string(),
        }
    }
    /// The `NFS_FF_TEST` glue reads the process environment, which a browser build has none of;
    /// there is nothing to drive here.
    pub fn rumble_test_tick(&mut self) {}
    pub fn haptic_device(&mut self) -> Option<bool> { None }
    pub fn haptic_arm(&mut self, _: i32) -> Option<bool> { None }
    pub fn haptic_armed(&self) -> bool { false }
    pub fn haptic_call(&mut self, _: &crate::ff::DiCall) {}
    pub fn haptic_stop_all(&mut self) {}
    pub fn haptic_describe(&mut self) -> String { "Wheel force feedback unavailable in browser".into() }
    pub fn photo_pad(&self) -> Option<PhotoPad> {
        let p = self.pad_snapshot()?;
        Some(PhotoPad { left:(photo_deadzone(p.sticks[0] as f32/32767.0), -photo_deadzone(p.sticks[1] as f32/32767.0)), right:(photo_deadzone(p.sticks[2] as f32/32767.0), -photo_deadzone(p.sticks[3] as f32/32767.0)), trigger_left:p.triggers[0] as f32/32767.0, trigger_right:p.triggers[1] as f32/32767.0, south:p.buttons[0],east:p.buttons[1],north:p.buttons[2],west:p.buttons[3],bumper_left:p.buttons[6],bumper_right:p.buttons[7],dpad_up:p.dpad[0],dpad_right:p.dpad[1],dpad_down:p.dpad[2],dpad_left:p.dpad[3],left_thumb:p.buttons[13],start:p.buttons[11] })
    }
}
