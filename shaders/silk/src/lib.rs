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

const SILK_SPEED: f32 = 6.0;
const SILK_SCALE: f32 = 1.5;
const SILK_NOISE_INTENSITY: f32 = 0.3;
const SILK_ROTATION: f32 = 0.0;
const SILK_TIME_SCALE: f32 = 0.1;
const SILK_COLOR_R: f32 = 82.0 / 255.0;
const SILK_COLOR_G: f32 = 39.0 / 255.0;
const SILK_COLOR_B: f32 = 255.0 / 255.0;

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

fn silk_noise(tex_coord: Vec2) -> f32 {
    const E: f32 = 2.718_281_7;
    let sx = (E * tex_coord.x).sin();
    let sy = (E * tex_coord.y).sin();
    let rx = E * sx;
    let ry = E * sy;
    (rx * ry * (1.0 + tex_coord.x)).fract()
}

fn rotate_uvs(uv: Vec2, angle: f32) -> Vec2 {
    let c = angle.cos();
    let s = angle.sin();
    Vec2::new(c * uv.x - s * uv.y, s * uv.x + c * uv.y)
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
    let uv = frag_xy / resolution;

    let rnd = silk_noise(frag_xy);
    let rotated_uv = rotate_uvs(uv * SILK_SCALE, SILK_ROTATION);
    let mut tex = rotated_uv * SILK_SCALE;
    let t_offset = SILK_SPEED * (uniforms.time_seconds * SILK_TIME_SCALE);

    tex.y += 0.03 * (8.0 * tex.x - t_offset).sin();

    let pattern = 0.6
        + 0.4
            * (5.0
                * (tex.x
                    + tex.y
                    + (3.0 * tex.x + 5.0 * tex.y).cos()
                    + 0.02 * t_offset)
                + (20.0 * (tex.x + tex.y - 0.1 * t_offset)).sin())
            .sin();

    let base = Vec3::new(SILK_COLOR_R, SILK_COLOR_G, SILK_COLOR_B) * pattern;
    let noise = Vec3::splat((rnd / 15.0) * SILK_NOISE_INTENSITY);
    let color = base - noise;

    *output = Vec4::new(color.x, color.y, color.z, 1.0);
}
