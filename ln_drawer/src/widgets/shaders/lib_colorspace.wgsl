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

// wgpu constant `transpose` is not implemented yet 

const oklab2lms = mat3x3f(
    1.0000000000, 1.0000000000, 1.0000000000,
    0.3963377774, -0.1055613458, -0.0894841775,
    0.2158037573, -0.0638541728, -1.2914855480,
);

const lms2rgb = mat3x3f(
     4.0767416621, -1.2684380046, -0.0041960863,
    -3.3077115913,  2.6097574011, -0.7034186147,
     0.2309699292, -0.3413193965,  1.7076147010,
);

const rgb2lms = mat3x3f(
    0.4122214708, 0.2119034982, 0.0883024619,
    0.5363325363, 0.6806995451, 0.2817188376,
    0.0514459929, 0.1073969566, 0.6299787005,
);

const lms2oklab = mat3x3f(
    0.2104542553, 1.9779984951, 0.0259040371,
    0.7936177850, -2.4285922050, 0.7827717662,
    -0.0040720468, 0.4505937099, -0.8086757660,
);

fn oklab_to_linear_srgb(oklab: vec3f) -> vec3f {
    // Oklab -> L'M'S'
    let lms_ = oklab2lms * oklab;
    
    // L'M'S' -> LMS
    let lms = lms_ * lms_ * lms_;
    
    // LMS -> sRGB
    return lms2rgb * lms;
}

fn linear_srgb_to_oklab(rgb: vec3f) -> vec3f {
    // linear sRGB -> LMS
    let lms = rgb2lms * rgb;
    
    // LMS -> L'M'S'
    let lms_ = pow(lms, vec3f(1.0 / 3.0));
    
    // L'M'S' -> Oklab
    return lms2oklab * lms_;
}

fn oklch_to_linear_srgb(lch: vec3f) -> vec3f {
    let a = lch.y * cos(lch.z);
    let b = lch.y * sin(lch.z);
    let oklab = vec3f(lch.x, a, b);
    return oklab_to_linear_srgb(oklab);
}

fn linear_srgb_to_oklch(rgb: vec3f) -> vec3f {
    let oklab = linear_srgb_to_oklab(rgb);
    let c = sqrt(oklab.y * oklab.y + oklab.z * oklab.z);
    let h = atan2(oklab.z, oklab.y);
    return vec3f(oklab.x, c, h);
}