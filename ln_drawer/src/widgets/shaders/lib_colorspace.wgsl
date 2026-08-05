fn srgb_to_linear(v: vec4f) -> vec4f {
    return vec4f(select(pow((v.rgb + 0.055) / 1.055, vec3(2.4)), v.rgb / 12.92, v.rgb < vec3(0.04045)), v.a);
}

fn linear_to_srgb(v: vec4f) -> vec4f {
    return vec4f(select(1.055 * pow(v.rgb, vec3(1.0 / 2.4)) - 0.055, v.rgb * 12.92, v.rgb < vec3(0.0031308)), v.a);
}

fn alpha_premultiplied_invert(v: vec4f) -> vec4f {
    return vec4f(select(v.rgb / v.a, vec3f(), v.a < 1e-6), v.a);
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3f {
    return l + s * (hue_to_rgb(h) - 0.5) * (1.0 - abs(2.0 * l - 1.0));
}

fn hue_to_rgb(h: f32) -> vec3f {
    return clamp(abs(((h * 6.0 + vec3f(0.0, 4.0, 2.0)) % 6.0) - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
}
