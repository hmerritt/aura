vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    float minimum_resolution = min(resolution.x, resolution.y);
    vec2 uv = (fragCoord * 2.0 - resolution) / minimum_resolution;
    float d = -uniforms.time_seconds * 0.8;
    float a = 0.0;
    for (int i = 0; i < 8; ++i) {
        float fi = float(i);
        a += cos(fi - d - a * uv.x);
        d += sin(uv.y * fi + a);
    }
    d += uniforms.time_seconds * 0.5;
    vec3 color = mix(vec3(0.0, 0.4, 1.0), vec3(1.0), cos(a) * 0.5 + 0.5);
    return vec4(color, 1.0);
}
