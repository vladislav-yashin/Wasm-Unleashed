//! GPU timestamp profiling is optional and disabled for portable browser rendering.
pub(crate) fn minimal() -> bool { false }
pub(crate) fn wanted() -> bool { false }
pub(crate) fn init(_: &wgpu::Device, _: &wgpu::Queue) {}
pub(crate) fn ctx(_: &'static str) {}
pub(crate) fn rp(_: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'static>> { None }
pub(crate) fn resolve(_: &mut wgpu::CommandEncoder) {}
pub(crate) fn end_frame(_: &wgpu::Device) {}
