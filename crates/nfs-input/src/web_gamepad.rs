use crate::{InputState, bindings::PadRecord};
use wasm_bindgen::JsCast;
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
#[derive(Default)]
pub struct Gamepads { pads: Vec<(String, PadSnapshot)>, previous: Vec<PadSnapshot>, hotplug: bool }
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
        input.set_pads(&self.records());
    }
    pub fn records(&self) -> Vec<PadRecord> { self.pads.iter().map(|(_, p)| pad_record(p)).collect() }
    pub fn records_into(&self, out: &mut Vec<PadRecord>) { *out = self.records(); }
    pub fn devices(&self) -> Vec<(String, bool)> { self.pads.iter().map(|(n,_)| (n.clone(),true)).collect() }
    pub fn axis_sets(&self) -> Vec<PadAxisSet> { self.records().iter().map(|r| PadAxisSet { mask:r.mask, values:r.axes }).collect() }
    pub fn pad_snapshot(&self) -> Option<PadSnapshot> { self.pads.first().map(|(_,p)| *p) }
    pub fn take_pad_hotplug(&mut self) -> bool { std::mem::take(&mut self.hotplug) }
    pub fn take_ff_hotplug(&mut self) -> bool { false }
    pub fn rumble_supported(&self) -> bool { false }
    pub fn rumble_sustain(&mut self, _: crate::ff::Motor, _: f32) {}
    pub fn rumble_pulse(&mut self, _: f32, _: u32) {}
    pub fn rumble_stop_all(&mut self) {}
    pub fn rumble_describe(&self) -> String { "Browser controller".into() }
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
