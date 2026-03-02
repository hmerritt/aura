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

fn mul_xy_macro(vec: Vec2, z: f32) -> Vec2 {
    let a = (z * 1.1).cos();
    let b = (z * 1.1 + 11.0).cos();
    let c = (z * 1.1 + 33.0).cos();
    Vec2::new(vec.x * a + vec.y * b, vec.x * c + vec.y * a)
}

fn n(p: Vec2) -> f32 {
    (p.x * 3.0 + (p.y * 2.7).sin()).sin() * (p.y * 1.1 + (p.x * 2.3).cos()).cos()
}

fn f(mut p: Vec3) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    for _ in 0..7 {
        value += n(Vec2::new(p.x, p.y) + Vec2::splat(p.z * 0.5)) * amplitude;
        p *= 2.0;
        amplitude *= 0.5;
    }
    value
}

fn m(mut p: Vec3, time: f32) -> f32 {
    let rotated = mul_xy_macro(Vec2::new(p.x, p.y), p.z);
    p.x = rotated.x;
    p.y = rotated.y;
    (1.0 - Vec2::new(p.x, p.y).length() - f(p + Vec3::splat(time * 0.1)) * 0.3) / 5.0
}

fn estimate_normal(p: Vec3, t: f32, time: f32) -> Vec3 {
    let e = 1e-3 + t * 1e-3;
    Vec3::new(
        m(p + Vec3::new(e, 0.0, 0.0), time) - m(p - Vec3::new(e, 0.0, 0.0), time),
        m(p + Vec3::new(0.0, e, 0.0), time) - m(p - Vec3::new(0.0, e, 0.0), time),
        m(p + Vec3::new(0.0, 0.0, e), time) - m(p - Vec3::new(0.0, 0.0, e), time),
    )
    .normalize()
}

fn ambient_occlusion(p: Vec3, normal: Vec3, time: f32) -> f32 {
    let mut occlusion = 0.0;
    let mut scale = 1.0;
    for i in 1..=5 {
        let h = 0.01 + 0.03 * i as f32;
        occlusion += (h - m(p + normal * h, time)) * scale;
        if occlusion > 0.33 {
            break;
        }
        scale *= 0.9;
    }
    (1.0 - 3.0 * occlusion).max(0.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[spirv(fragment)]
pub fn fs_main(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &ShaderUniforms,
    output: &mut Vec4,
) {
    let time = uniforms.time_seconds;
    let resolution = Vec2::new(
        uniforms.resolution.x.max(1.0),
        uniforms.resolution.y.max(1.0),
    );
    let uv = Vec2::new(frag_coord.x, frag_coord.y);

    let ray_dir = Vec3::new(
        uv.x - 0.5 * resolution.x,
        uv.y - 0.5 * resolution.y,
        resolution.y,
    )
    .normalize();
    let origin = Vec3::new(0.0, 0.0, time);

    let mut t = 0.0;
    let mut p = origin;
    for _ in 0..99 {
        p = origin + ray_dir * t;
        let w = m(p, time);
        if w.abs() < t * 1e-3 || t > 20.0 {
            break;
        }
        t += w;
    }

    let mut c = Vec3::new(0.0, 0.0, 0.0);
    if t <= 20.0 {
        let normal = estimate_normal(p, t, time);

        let mut q = p;
        let rotated = mul_xy_macro(Vec2::new(q.x, q.y), q.z);
        q.x = rotated.x;
        q.y = rotated.y;

        let base = mix_vec3(
            Vec3::new(0.1, 0.3, 0.7),
            Vec3::new(0.8, 0.4, 0.2),
            (f(q + Vec3::splat(time * 0.1)) + 0.5).clamp(0.0, 1.0),
        );
        let light_vec = origin + Vec3::new(0.0, 0.0, 4.0) - p;
        let l = light_vec.normalize();
        let h = (l + (origin - p).normalize()).normalize();
        let w = light_vec.length();

        let diffuse = normal.dot(l).max(0.0);
        let specular = normal.dot(h).max(0.0).abs().powf(16.0) * smoothstep(15.0, 5.0, t);
        let lighting = (base * diffuse + Vec3::splat(0.8) * specular) / (1.0 + w * w / 5.0);

        c = base * 0.02 + lighting;
        c *= ambient_occlusion(p, normal, time);
    }

    c = mix_vec3(Vec3::new(0.02, 0.0, 0.05), c, (-0.15 * t).exp());
    c = c * (c * 2.51 + Vec3::splat(0.03))
        / (c * (c * 2.43 + Vec3::splat(0.59)) + Vec3::splat(0.14));

    c = Vec3::new(
        c.x.abs().powf(1.0 / 2.2),
        c.y.abs().powf(1.0 / 2.2),
        c.z.abs().powf(1.0 / 2.2),
    );
    *output = Vec4::new(c.x, c.y, c.z, 1.0);
}
