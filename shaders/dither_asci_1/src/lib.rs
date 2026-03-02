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

fn scalar_step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}

fn mix_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

fn synthesize_character(uv: Vec2, luminance: f32) -> f32 {
    let p = uv * 2.0 - Vec2::splat(1.0);
    let tier = (luminance * 5.0).clamp(0.0, 4.0) as i32;

    if tier == 0 {
        0.0
    } else if tier == 1 {
        1.0 - scalar_step(0.2, p.length())
    } else if tier == 2 {
        let mut density = 1.0 - scalar_step(0.2, p.y.abs());
        density *= 1.0 - scalar_step(0.6, p.x.abs());
        density
    } else if tier == 3 {
        let horizontal = (1.0 - scalar_step(0.2, p.y.abs())) * (1.0 - scalar_step(0.6, p.x.abs()));
        let vertical = (1.0 - scalar_step(0.2, p.x.abs())) * (1.0 - scalar_step(0.6, p.y.abs()));
        horizontal.max(vertical)
    } else {
        let h1 = 1.0 - scalar_step(0.15, (p.y - 0.3).abs());
        let h2 = 1.0 - scalar_step(0.15, (p.y + 0.3).abs());
        let v1 = 1.0 - scalar_step(0.15, (p.x - 0.3).abs());
        let v2 = 1.0 - scalar_step(0.15, (p.x + 0.3).abs());
        let mut density = h1.max(h2).max(v1.max(v2));
        density *= 1.0 - scalar_step(0.8, p.x.abs().max(p.y.abs()));
        density
    }
}

fn compute_bayer(cell: Vec2) -> f32 {
    let x = (cell.x as i32) & 3;
    let y = (cell.y as i32) & 3;
    let idx = x + y * 4;

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
    let grid_resolution = 10.0;

    let grid_pos = frag_xy / grid_resolution;
    let cell_index = grid_pos.floor();
    let local_uv = grid_pos.fract();

    let mut uv = (cell_index * grid_resolution) / resolution;
    uv = uv * 2.0 - Vec2::splat(1.0);
    uv.x *= resolution.x / resolution.y;

    let time = uniforms.time_seconds * 0.5;
    let wave1 = (uv.x * 5.0 + time).sin();
    let wave2 = (uv.y * 5.0 + time).sin();
    let wave3 = (uv.x * uv.y * 10.0 - time).sin();
    let wave4 = (uv.length() * 10.0 - time * 2.0).sin();
    let base_luminance = (wave1 + wave2 + wave3 + wave4) * 0.25 + 0.5;

    let dither_threshold = compute_bayer(cell_index);
    let adjusted_luminance = (base_luminance + (dither_threshold - 0.5) * 0.4).clamp(0.0, 1.0);

    let mask = synthesize_character(local_uv, adjusted_luminance);
    let color_low = Vec3::new(0.0, 0.1, 0.2);
    let color_high = Vec3::new(0.2, 0.9, 0.5);
    let final_color = mix_vec3(color_low, color_high, base_luminance);

    *output = Vec4::new(
        final_color.x * mask,
        final_color.y * mask,
        final_color.z * mask,
        1.0,
    );
}
