float aura_silk_noise(vec2 tex_coord) {
    const float e = 2.7182817;
    vec2 value = e * sin(e * tex_coord);
    float noise = value.x * value.y * (1.0 + tex_coord.x);
    return noise - trunc(noise);
}

vec2 aura_silk_rotate(vec2 uv, float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return vec2(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
}

vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    vec2 uv = fragCoord / resolution;
    float random_value = aura_silk_noise(fragCoord);
    vec2 tex = aura_silk_rotate(uv * 1.5, 0.0) * 1.5;
    float time_offset = 6.0 * (uniforms.time_seconds * 0.1);
    tex.y += 0.03 * sin(8.0 * tex.x - time_offset);
    float pattern = 0.6 + 0.4 * sin(5.0 * (tex.x + tex.y + cos(3.0 * tex.x + 5.0 * tex.y) + 0.02 * time_offset) + sin(20.0 * (tex.x + tex.y - 0.1 * time_offset)));
    vec3 color = vec3(82.0 / 255.0, 39.0 / 255.0, 1.0) * pattern - vec3((random_value / 15.0) * 0.3);
    return vec4(color, 1.0);
}
