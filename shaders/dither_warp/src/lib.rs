#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec2, Vec3, Vec4};
use spirv_std::num_traits::Float;
use spirv_std::spirv;

#[repr(C)]
pub struct ShaderUniforms {
    pub time_seconds: f32,
    pub frame_index: u32,
    pub mouse_enabled: u32,
    pub _padding: u32,
    pub resolution: Vec4,
    pub mouse: Vec4,
}

// Hardcoded shader settings.
const COLOR_BACK: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const COLOR_FRONT: Vec3 = Vec3::new(19.0 / 255.0, 201.0 / 255.0, 49.0 / 255.0);
const SIZE: f32 = 2.2;
const SPEED: f32 = 1.0;
const SCALE: f32 = 1.4;
const ROTATION_DEG: f32 = 0.0;
const OFFSET_X: f32 = 0.0;
const OFFSET_Y: f32 = 0.0;

#[spirv(vertex)]
pub fn vs_main(#[spirv(vertex_index)] vertex_index: i32, #[spirv(position)] out_pos: &mut Vec4) {
    let x = match vertex_index {
        0 => -1.0,
        1 => 3.0,
        _ => -1.0,
    };
    let y = match vertex_index {
        0 => -1.0,
        1 => -1.0,
        _ => 3.0,
    };
    *out_pos = Vec4::new(x, y, 0.0, 1.0);
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn scalar_step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}

fn rotate_uv(uv: Vec2, angle_rad: f32) -> Vec2 {
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    Vec2::new(c * uv.x - s * uv.y, s * uv.x + c * uv.y)
}

fn bayer_4x4(uv: Vec2) -> f32 {
    let x = (uv.x.floor() as i32).rem_euclid(4);
    let y = (uv.y.floor() as i32).rem_euclid(4);
    let idx = y * 4 + x;

    match idx {
        0 => 0.0 / 16.0,
        1 => 8.0 / 16.0,
        2 => 2.0 / 16.0,
        3 => 10.0 / 16.0,
        4 => 12.0 / 16.0,
        5 => 4.0 / 16.0,
        6 => 14.0 / 16.0,
        7 => 6.0 / 16.0,
        8 => 3.0 / 16.0,
        9 => 11.0 / 16.0,
        10 => 1.0 / 16.0,
        11 => 9.0 / 16.0,
        12 => 15.0 / 16.0,
        13 => 7.0 / 16.0,
        14 => 13.0 / 16.0,
        _ => 5.0 / 16.0,
    }
}

#[spirv(fragment)]
pub fn fs_main(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &ShaderUniforms,
    output: &mut Vec4,
) {
    let resolution = Vec2::new(
        uniforms.resolution.x.max(1.0),
        uniforms.resolution.y.max(1.0),
    );
    let frag_xy = Vec2::new(frag_coord.x, frag_coord.y);

    let t = 0.5 * uniforms.time_seconds * SPEED;
    let px_size = SIZE.max(0.001);

    let px_size_uv = (frag_xy - 0.5 * resolution) / px_size;
    let canvas_pixelized_uv = (px_size_uv.floor() + Vec2::splat(0.5)) * px_size;

    // Keep warp domain in pixel space so SIZE affects the pattern like the TS shader.
    let mut shape_uv = canvas_pixelized_uv;
    shape_uv += Vec2::new(-OFFSET_X, OFFSET_Y);
    shape_uv /= SCALE.max(0.001);

    let rotation_rad = ROTATION_DEG.to_radians();
    shape_uv = rotate_uv(shape_uv, rotation_rad);

    // Warp shape (u_shape = 2).
    shape_uv *= 0.003;
    for i in 1..6 {
        let i_f = i as f32;
        shape_uv.x += 0.6 / i_f * (i_f * 2.5 * shape_uv.y + t).cos();
        shape_uv.y += 0.6 / i_f * (i_f * 1.5 * shape_uv.x + t).cos();
    }

    let mut shape = 0.15 / (t - shape_uv.y - shape_uv.x).sin().abs().max(0.001);
    shape = smoothstep(0.02, 1.0, shape);

    // 4x4 Bayer dithering only (u_type = 3).
    let dithering = bayer_4x4(px_size_uv) - 0.5;
    let res = scalar_step(0.5, shape + dithering);

    let color = COLOR_FRONT * res + COLOR_BACK * (1.0 - res);
    *output = Vec4::new(color.x, color.y, color.z, 1.0);
}
