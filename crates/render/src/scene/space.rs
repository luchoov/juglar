//! Three-dimensional space for the scenes that are places (Citadel, Bore):
//! vectors as plain arrays, and the camera handed to the shader.
//!
//! These scenes draw no waveform, so the camera travels in the waveform's
//! slots: 0–2 where it is, 4–6 where it looks, 8–10 its up, 12–14 its
//! right. Each shader reads them back with its own `camera(i)`.

use juglar_signals::WAVEFORM_POINTS;

pub(crate) type V3 = [f32; 3];

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub(crate) fn normalize(a: V3) -> V3 {
    scale(a, 1.0 / dot(a, a).sqrt().max(1e-9))
}

/// The direction `v` turned by `angle` radians about `axis` (unit).
pub(crate) fn turned(v: V3, axis: V3, angle: f32) -> V3 {
    let (s, c) = angle.sin_cos();
    add(add(scale(v, c), scale(cross(axis, v), s)), scale(axis, dot(axis, v) * (1.0 - c)))
}

/// A camera at `position` looking along `forward` (unit), its `up` (unit,
/// across `forward`) turned by `roll` radians: written into the slots the
/// shader reads. The slots after 15 are the scene's own.
pub(crate) fn camera(position: V3, forward: V3, up: V3, roll: f32) -> [f32; WAVEFORM_POINTS] {
    let right = normalize(cross(forward, up));
    let (rs, rc) = roll.sin_cos();
    let rolled_up = add(scale(up, rc), scale(right, rs));
    let rolled_right = add(scale(right, rc), scale(up, -rs));
    let mut slots = [0.0; WAVEFORM_POINTS];
    slots[..3].copy_from_slice(&position);
    slots[4..7].copy_from_slice(&forward);
    slots[8..11].copy_from_slice(&rolled_up);
    slots[12..15].copy_from_slice(&rolled_right);
    slots
}
