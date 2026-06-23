fn srgb_to_linear(v: vec4f) -> vec4f {
    let threshold = vec3(0.04045);
    let low = v.rgb / 12.92;
    let high = pow((v.rgb + 0.055) / 1.055, vec3(2.4));
    return vec4f(select(high, low, v.rgb < threshold), v.a);
}

fn linear_to_srgb(v: vec4f) -> vec4f {
    let threshold = vec3(0.0031308);
    let low = v.rgb * 12.92;
    let high = 1.055 * pow(v.rgb, vec3(1.0 / 2.4)) - 0.055;
    return vec4f(select(high, low, v.rgb < threshold), v.a);
}
