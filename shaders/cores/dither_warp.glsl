float aura_bayer_warp(vec2 uv) {
    float x = mod(mod(floor(uv.x), 4.0) + 4.0, 4.0);
    float y = mod(mod(floor(uv.y), 4.0) + 4.0, 4.0);
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

vec2 aura_rotate(vec2 uv, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return vec2(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
}

vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    const vec3 color_back = vec3(0.0);
    const vec3 color_front = vec3(19.0 / 255.0, 201.0 / 255.0, 49.0 / 255.0);
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    float time = 0.5 * uniforms.time_seconds;
    float pixel_size = 2.2;
    vec2 pixel_uv = (fragCoord - 0.5 * resolution) / pixel_size;
    vec2 shape_uv = (floor(pixel_uv) + vec2(0.5)) * pixel_size / 1.4;
    shape_uv = aura_rotate(shape_uv, 0.0) * 0.003;
    for (int i = 1; i < 6; ++i) {
        float fi = float(i);
        shape_uv.x += 0.6 / fi * cos(fi * 2.5 * shape_uv.y + time);
        shape_uv.y += 0.6 / fi * cos(fi * 1.5 * shape_uv.x + time);
    }
    float shape = 0.15 / max(abs(sin(time - shape_uv.y - shape_uv.x)), 0.001);
    shape = smoothstep(0.02, 1.0, shape);
    float result = step(0.5, shape + aura_bayer_warp(pixel_uv) - 0.5);
    return vec4(color_front * result + color_back * (1.0 - result), 1.0);
}
