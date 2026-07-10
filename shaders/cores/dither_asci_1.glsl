float aura_character(vec2 uv, float luminance) {
    vec2 p = uv * 2.0 - vec2(1.0);
    int tier = int(clamp(luminance * 5.0, 0.0, 4.0));
    if (tier == 0) return 0.0;
    if (tier == 1) return 1.0 - step(0.2, length(p));
    if (tier == 2) {
        return (1.0 - step(0.2, abs(p.y))) * (1.0 - step(0.6, abs(p.x)));
    }
    if (tier == 3) {
        float horizontal = (1.0 - step(0.2, abs(p.y))) * (1.0 - step(0.6, abs(p.x)));
        float vertical = (1.0 - step(0.2, abs(p.x))) * (1.0 - step(0.6, abs(p.y)));
        return max(horizontal, vertical);
    }
    float h1 = 1.0 - step(0.15, abs(p.y - 0.3));
    float h2 = 1.0 - step(0.15, abs(p.y + 0.3));
    float v1 = 1.0 - step(0.15, abs(p.x - 0.3));
    float v2 = 1.0 - step(0.15, abs(p.x + 0.3));
    return max(max(h1, h2), max(v1, v2)) * (1.0 - step(0.8, max(abs(p.x), abs(p.y))));
}

float aura_bayer_4x4(vec2 cell) {
    float x = mod(mod(floor(cell.x), 4.0) + 4.0, 4.0);
    float y = mod(mod(floor(cell.y), 4.0) + 4.0, 4.0);
    if (y < 0.5) {
        if (x < 0.5) return 0.0 / 16.0;
        if (x < 1.5) return 8.0 / 16.0;
        if (x < 2.5) return 2.0 / 16.0;
        return 10.0 / 16.0;
    }
    if (y < 1.5) {
        if (x < 0.5) return 12.0 / 16.0;
        if (x < 1.5) return 4.0 / 16.0;
        if (x < 2.5) return 14.0 / 16.0;
        return 6.0 / 16.0;
    }
    if (y < 2.5) {
        if (x < 0.5) return 3.0 / 16.0;
        if (x < 1.5) return 11.0 / 16.0;
        if (x < 2.5) return 1.0 / 16.0;
        return 9.0 / 16.0;
    }
    if (x < 0.5) return 15.0 / 16.0;
    if (x < 1.5) return 7.0 / 16.0;
    if (x < 2.5) return 13.0 / 16.0;
    return 5.0 / 16.0;
}

vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    float grid_size = 10.0;
    vec2 grid_pos = fragCoord / grid_size;
    vec2 cell_index = floor(grid_pos);
    vec2 local_uv = grid_pos - trunc(grid_pos);
    vec2 uv = cell_index * grid_size / resolution * 2.0 - vec2(1.0);
    uv.x *= resolution.x / resolution.y;
    float time = uniforms.time_seconds * 0.5;
    float base_luminance = (
        sin(uv.x * 5.0 + time) +
        sin(uv.y * 5.0 + time) +
        sin(uv.x * uv.y * 10.0 - time) +
        sin(length(uv) * 10.0 - time * 2.0)
    ) * 0.25 + 0.5;
    float adjusted = clamp(base_luminance + (aura_bayer_4x4(cell_index) - 0.5) * 0.4, 0.0, 1.0);
    float mask = aura_character(local_uv, adjusted);
    vec3 color = mix(vec3(0.0, 0.1, 0.2), vec3(0.2, 0.9, 0.5), base_luminance);
    return vec4(color * mask, 1.0);
}
