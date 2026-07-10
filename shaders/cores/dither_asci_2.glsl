float aura_char_mask(float intensity) {
    int index = int(clamp(intensity, 0.0, 1.0) * 7.0);
    if (index == 0) return 0.0;
    if (index == 1) return 2.0;
    if (index == 2) return 34.0;
    if (index == 3) return 328.0;
    if (index == 4) return 2976.0;
    if (index == 5) return 28662.0;
    if (index == 6) return 63903.0;
    return 65535.0;
}

float aura_sample_character(vec2 local_uv, float mask) {
    vec2 p = floor(local_uv * 4.0);
    if (p.x < 0.0 || p.x > 3.0 || p.y < 0.0 || p.y > 3.0) return 0.0;
    float bit_pos = p.x + p.y * 4.0;
    return floor(mod(floor(mask / exp2(bit_pos)), 2.0));
}

float aura_ascii_luminance(vec2 frag_coord, vec2 resolution, float time, vec2 mouse) {
    float font_size = 8.0;
    vec2 grid_scaled = frag_coord / font_size;
    vec2 grid_pos = floor(grid_scaled);
    vec2 local_uv = fract(grid_scaled);
    vec2 grid_center = grid_pos * font_size + vec2(font_size * 0.5);
    vec2 uv = grid_center / resolution * 2.0 - vec2(1.0);
    uv.x *= resolution.x / resolution.y;
    float phase_time = time * 0.8;
    float intensity = (sin(uv.x * 6.0 + phase_time) +
        sin(uv.y * 4.0 - phase_time) +
        sin(uv.x * uv.y * 5.0 + phase_time * 1.5)) / 3.0 * 0.5 + 0.5;
    vec2 mouse_pos = mouse;
    if (mouse_pos.x <= 0.0 && mouse_pos.y <= 0.0) mouse_pos = resolution * 0.5;
    float destruction = smoothstep(0.0, 100.0, distance(grid_center, mouse_pos));
    float final_intensity = intensity * destruction;
    return aura_sample_character(local_uv, aura_char_mask(final_intensity)) * final_intensity;
}

float aura_glass_height(vec2 uv) {
    vec2 sheared = vec2(uv.x + uv.y * 0.15, uv.x * -0.15 + uv.y) * 4.0;
    vec2 p = fract(sheared) - vec2(0.5);
    return smoothstep(0.48, 0.38, max(abs(p.x), abs(p.y)));
}

vec3 aura_glass_normal(vec2 uv) {
    vec2 e = vec2(0.002, 0.0);
    float height = aura_glass_height(uv);
    return normalize(vec3(height - aura_glass_height(uv + e), height - aura_glass_height(uv + e.yx), 0.02));
}

vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    vec2 uv = fragCoord / resolution;
    vec2 mouse = uniforms.mouse.xy;
    float height = aura_glass_height(uv);
    vec3 normal = aura_glass_normal(uv);
    vec2 offset = normal.xy * 40.0;
    float lum_r = aura_ascii_luminance(fragCoord - offset, resolution, uniforms.time_seconds, mouse);
    float lum_g = aura_ascii_luminance(fragCoord - offset * 1.05, resolution, uniforms.time_seconds, mouse);
    float lum_b = aura_ascii_luminance(fragCoord - offset * 1.15, resolution, uniforms.time_seconds, mouse);
    vec3 base_color = vec3(0.15, 0.85, 0.4) * 1.5;
    vec3 color = vec3(mix(0.02, base_color.r, lum_r), mix(0.05, base_color.g, lum_g), mix(0.02, base_color.b, lum_b));
    vec3 light_dir = normalize(vec3(0.5, 0.8, 1.0));
    vec3 half_vector = normalize(light_dir + vec3(0.0, 0.0, 1.0));
    float specular = pow(max(dot(normal, half_vector), 0.0), 64.0);
    color += vec3(specular * smoothstep(0.1, 0.9, height));
    color *= mix(0.1, 1.0, smoothstep(0.0, 0.1, height));
    return vec4(color, 1.0);
}
