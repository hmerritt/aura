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

#[spirv(vertex)]
pub fn vs_main(
    #[spirv(vertex_index)] vertex_index: i32,
    #[spirv(position)] out_pos: &mut Vec4,
) {
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

#[spirv(fragment)]
pub fn fs_main(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &ShaderUniforms,
    output: &mut Vec4,
) {
    let resolution = Vec2::new(uniforms.resolution.x.max(1.0), uniforms.resolution.y.max(1.0));
    let mr = resolution.x.min(resolution.y);
    let uv = (Vec2::new(frag_coord.x, frag_coord.y) * 2.0 - resolution) / mr;

    let mut d = -uniforms.time_seconds * 0.8;
    let mut a = 0.0;
    for i in 0..8 {
        let i_f = i as f32;
        a += (i_f - d - a * uv.x).cos();
        d += (uv.y * i_f + a).sin();
    }
    d += uniforms.time_seconds * 0.5;
    let _ = d;

    let color_a = Vec3::new(0.0, 0.4, 1.0);
    let color_b = Vec3::new(1.0, 1.0, 1.0);
    let t = a.cos() * 0.5 + 0.5;
    let col = color_a + (color_b - color_a) * t;
    *output = Vec4::new(col.x, col.y, col.z, 1.0);
}
