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

fn oklab_clip_to_srgb_gamut(oklab: vec3f) -> vec3f {
    let l = clamp(oklab.x, 0.0, 1.0);
    let c = max(sqrt(oklab.y * oklab.y + oklab.z * oklab.z), 1e-5);
    let t = min(find_gamut_intersection(oklab.y / c, oklab.z / c, l, c, l), 1.0);
    return vec3f(l, t * oklab.y, t * oklab.z);
}

// sRGB gamut clipping.
// Ported from Björn Ottosson's "sRGB gamut clipping".
// https://bottosson.github.io/posts/gamutclipping/
// Rust reference: ln_drawer/src/widgets/palette/utils.rs

fn compute_max_saturation(a: f32, b: f32) -> f32 {
    var k0: f32;
    var k1: f32;
    var k2: f32;
    var k3: f32;
    var k4: f32;
    var wl: f32;
    var wm: f32;
    var ws: f32;

    if -1.88170328 * a - 0.80936493 * b > 1.0 {
        k0 = 1.19086277;
        k1 = 1.76576728;
        k2 = 0.59662641;
        k3 = 0.75515197;
        k4 = 0.56771245;
        wl = 4.0767416621;
        wm = -3.3077115913;
        ws = 0.2309699292;
    } else if 1.81444104 * a - 1.19445276 * b > 1.0 {
        k0 = 0.73956515;
        k1 = -0.45954404;
        k2 = 0.08285427;
        k3 = 0.12541070;
        k4 = 0.14503204;
        wl = -1.2684380046;
        wm = 2.6097574011;
        ws = -0.3413193965;
    } else {
        k0 = 1.35733652;
        k1 = -0.00915799;
        k2 = -1.15130210;
        k3 = -0.50559606;
        k4 = 0.00692167;
        wl = -0.0041960863;
        wm = -0.7034186147;
        ws = 1.7076147010;
    }

    let s = k0 + k1 * a + k2 * b + k3 * a * a + k4 * a * b;

    let k_l = 0.3963377774 * a + 0.2158037573 * b;
    let k_m = -0.1055613458 * a - 0.0638541728 * b;
    let k_s = -0.0894841775 * a - 1.2914855480 * b;

    let l_p = 1.0 + s * k_l;
    let m_p = 1.0 + s * k_m;
    let s_p = 1.0 + s * k_s;

    let l_c = l_p * l_p * l_p;
    let m_c = m_p * m_p * m_p;
    let s_c = s_p * s_p * s_p;

    let l_ds = 3.0 * k_l * l_p * l_p;
    let m_ds = 3.0 * k_m * m_p * m_p;
    let s_ds = 3.0 * k_s * s_p * s_p;

    let l_ds2 = 6.0 * k_l * k_l * l_p;
    let m_ds2 = 6.0 * k_m * k_m * m_p;
    let s_ds2 = 6.0 * k_s * k_s * s_p;

    let f = wl * l_c + wm * m_c + ws * s_c;
    let f1 = wl * l_ds + wm * m_ds + ws * s_ds;
    let f2 = wl * l_ds2 + wm * m_ds2 + ws * s_ds2;

    return s - f * f1 / (f1 * f1 - 0.5 * f * f2);
}

fn find_cusp(a: f32, b: f32) -> vec2f {
    let s_cusp = compute_max_saturation(a, b);
    let rgb = oklab_to_linear_srgb(vec3f(1.0, s_cusp * a, s_cusp * b));
    let l_cusp = pow(1.0 / max(rgb.r, max(rgb.g, rgb.b)), 1.0 / 3.0);
    return vec2f(l_cusp, l_cusp * s_cusp);
}

fn find_gamut_intersection(a: f32, b: f32, l1: f32, c1: f32, l0: f32) -> f32 {
    let cusp = find_cusp(a, b);
    let cusp_l = cusp.x;
    let cusp_c = cusp.y;

    var t: f32;
    if (l1 - l0) * cusp_c - (cusp_l - l0) * c1 <= 0.0 {
        t = cusp_c * l0 / (c1 * cusp_l + cusp_c * (l0 - l1));
    } else {
        t = cusp_c * (l0 - 1.0) / (c1 * (cusp_l - 1.0) + cusp_c * (l0 - l1));

        let d_l = l1 - l0;
        let d_c = c1;

        let k_l = 0.3963377774 * a + 0.2158037573 * b;
        let k_m = -0.1055613458 * a - 0.0638541728 * b;
        let k_s = -0.0894841775 * a - 1.2914855480 * b;

        let l_dt = d_l + d_c * k_l;
        let m_dt = d_l + d_c * k_m;
        let s_dt = d_l + d_c * k_s;

        let l_p = l0 * (1.0 - t) + t * l1 + t * c1 * k_l;
        let m_p = l0 * (1.0 - t) + t * l1 + t * c1 * k_m;
        let s_p = l0 * (1.0 - t) + t * l1 + t * c1 * k_s;

        let l_c = l_p * l_p * l_p;
        let m_c = m_p * m_p * m_p;
        let s_c = s_p * s_p * s_p;

        let ldt = 3.0 * l_dt * l_p * l_p;
        let mdt = 3.0 * m_dt * m_p * m_p;
        let sdt = 3.0 * s_dt * s_p * s_p;

        let ldt2 = 6.0 * l_dt * l_dt * l_p;
        let mdt2 = 6.0 * m_dt * m_dt * m_p;
        let sdt2 = 6.0 * s_dt * s_dt * s_p;

        let r = 4.0767416621 * l_c - 3.3077115913 * m_c + 0.2309699292 * s_c - 1.0;
        let r1 = 4.0767416621 * ldt - 3.3077115913 * mdt + 0.2309699292 * sdt;
        let r2 = 4.0767416621 * ldt2 - 3.3077115913 * mdt2 + 0.2309699292 * sdt2;

        let u_r = r1 / (r1 * r1 - 0.5 * r * r2);
        var t_r = -r * u_r;

        let g = -1.2684380046 * l_c + 2.6097574011 * m_c - 0.3413193965 * s_c - 1.0;
        let g1 = -1.2684380046 * ldt + 2.6097574011 * mdt - 0.3413193965 * sdt;
        let g2 = -1.2684380046 * ldt2 + 2.6097574011 * mdt2 - 0.3413193965 * sdt2;

        let u_g = g1 / (g1 * g1 - 0.5 * g * g2);
        var t_g = -g * u_g;

        let b_ = -0.0041960863 * l_c - 0.7034186147 * m_c + 1.7076147010 * s_c - 1.0;
        let b1 = -0.0041960863 * ldt - 0.7034186147 * mdt + 1.7076147010 * sdt;
        let b2 = -0.0041960863 * ldt2 - 0.7034186147 * mdt2 + 1.7076147010 * sdt2;

        let u_b = b1 / (b1 * b1 - 0.5 * b_ * b2);
        var t_b = -b_ * u_b;

        if u_r < 0.0 {
            t_r = 3.4028234663852886e+38;
        }
        if u_g < 0.0 {
            t_g = 3.4028234663852886e+38;
        }
        if u_b < 0.0 {
            t_b = 3.4028234663852886e+38;
        }

        t += min(t_r, min(t_g, t_b));
    }

    return t;
}