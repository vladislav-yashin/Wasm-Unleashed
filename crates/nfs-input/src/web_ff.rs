pub use crate::rumbler::Motor;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiSlot {
    /// slot 0, `GUID_ConstantForce` — the collision jolt
    Constant,
    /// slot 2, `GUID_Square` — the road rumble
    Square,
    /// slot 4, `GUID_Sine` — the engine rumble
    Sine,
    /// slot 6, `GUID_Spring` (two conditions) — the grip / self-aligning spring
    Spring,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiCall {
    Stop(DiSlot),
    /// `DIPERIODIC {magnitude, 0, 0, period_us}` on the square or the sine, infinite duration
    Periodic { slot: DiSlot, magnitude: i32, period_us: i32, gain: i32 },
    /// two `DICONDITION`s `{offset, coef, coef, coef, coef, 0}` (axes X, Y), infinite duration
    Spring { x: (i32, i32), y: (i32, i32), gain: i32 },
    /// `DICONSTANTFORCE {magnitude}`, `duration_us`, polar `direction` in degrees, `DIENVELOPE`
    Constant { magnitude: i32, duration_us: i32, direction: i32, attack_level: i32, attack_time: i32, fade_level: i32, fade_time: i32, gain: i32 },
}
