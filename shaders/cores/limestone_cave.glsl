vec2 aura_cave_rotate(vec2 value, float z) {
    float a = cos(z * 1.1);
    float b = cos(z * 1.1 + 11.0);
    float c = cos(z * 1.1 + 33.0);
    return vec2(value.x * a + value.y * b, value.x * c + value.y * a);
}

float aura_cave_noise(vec2 p) {
    return sin(p.x * 3.0 + sin(p.y * 2.7)) * cos(p.y * 1.1 + cos(p.x * 2.3));
}

float aura_cave_fractal(vec3 p) {
    float value = 0.0;
    float amplitude = 1.0;
    for (int i = 0; i < 7; ++i) {
        value += aura_cave_noise(p.xy + vec2(p.z * 0.5)) * amplitude;
        p *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

float aura_cave_map(vec3 p, float time) {
    p.xy = aura_cave_rotate(p.xy, p.z);
    return (1.0 - length(p.xy) - aura_cave_fractal(p + vec3(time * 0.1)) * 0.3) / 5.0;
}

vec3 aura_cave_normal(vec3 p, float distance_value, float time) {
    float e = 0.001 + distance_value * 0.001;
    return normalize(vec3(
        aura_cave_map(p + vec3(e, 0.0, 0.0), time) - aura_cave_map(p - vec3(e, 0.0, 0.0), time),
        aura_cave_map(p + vec3(0.0, e, 0.0), time) - aura_cave_map(p - vec3(0.0, e, 0.0), time),
        aura_cave_map(p + vec3(0.0, 0.0, e), time) - aura_cave_map(p - vec3(0.0, 0.0, e), time)
    ));
}

float aura_cave_ao(vec3 p, vec3 normal, float time) {
    float occlusion = 0.0;
    float scale = 1.0;
    for (int i = 1; i <= 5; ++i) {
        float h = 0.01 + 0.03 * float(i);
        occlusion += (h - aura_cave_map(p + normal * h, time)) * scale;
        if (occlusion > 0.33) break;
        scale *= 0.9;
    }
    return max(1.0 - 3.0 * occlusion, 0.0);
}

vec4 aura_main(vec2 fragCoord, AuraUniforms uniforms) {
    float time = uniforms.time_seconds;
    vec2 resolution = max(uniforms.resolution.xy, vec2(1.0));
    vec3 ray_dir = normalize(vec3(fragCoord - 0.5 * resolution, resolution.y));
    vec3 origin = vec3(0.0, 0.0, time);
    float travel = 0.0;
    vec3 p = origin;
    for (int i = 0; i < 99; ++i) {
        p = origin + ray_dir * travel;
        float step_distance = aura_cave_map(p, time);
        if (abs(step_distance) < travel * 0.001 || travel > 20.0) break;
        travel += step_distance;
    }
    vec3 color = vec3(0.0);
    if (travel <= 20.0) {
        vec3 normal = aura_cave_normal(p, travel, time);
        vec3 q = p;
        q.xy = aura_cave_rotate(q.xy, q.z);
        vec3 base = mix(vec3(0.1, 0.3, 0.7), vec3(0.8, 0.4, 0.2), clamp(aura_cave_fractal(q + vec3(time * 0.1)) + 0.5, 0.0, 1.0));
        vec3 light_vector = origin + vec3(0.0, 0.0, 4.0) - p;
        vec3 light = normalize(light_vector);
        vec3 half_vector = normalize(light + normalize(origin - p));
        float diffuse = max(dot(normal, light), 0.0);
        float specular = pow(abs(max(dot(normal, half_vector), 0.0)), 16.0) * smoothstep(15.0, 5.0, travel);
        vec3 lighting = (base * diffuse + vec3(0.8) * specular) / (1.0 + dot(light_vector, light_vector) / 5.0);
        color = (base * 0.02 + lighting) * aura_cave_ao(p, normal, time);
    }
    color = mix(vec3(0.02, 0.0, 0.05), color, exp(-0.15 * travel));
    color = color * (color * 2.51 + vec3(0.03)) / (color * (color * 2.43 + vec3(0.59)) + vec3(0.14));
    color = pow(abs(color), vec3(1.0 / 2.2));
    return vec4(color, 1.0);
}
