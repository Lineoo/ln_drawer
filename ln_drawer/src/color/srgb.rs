use glam::Vec4;

/// x -> S(x)
pub fn srgb_gamma_encode(v: Vec4) -> Vec4 {
    Vec4::new(
        srgb_gamma_encode_f32(v.x),
        srgb_gamma_encode_f32(v.y),
        srgb_gamma_encode_f32(v.z),
        v.w,
    )
}

/// S(x) -> x
pub fn srgb_gamma_decode(v: Vec4) -> Vec4 {
    Vec4::new(
        srgb_gamma_decode_f32(v.x),
        srgb_gamma_decode_f32(v.y),
        srgb_gamma_decode_f32(v.z),
        v.w,
    )
}

/// x -> S(x) (single channel version)
pub fn srgb_gamma_encode_f32(v: f32) -> f32 {
    match v < 0.0031308 {
        true => v * 12.92,
        false => 1.055 * v.powf(1.0 / 2.4) - 0.055,
    }
}

/// S(x) -> x (single channel version)
pub fn srgb_gamma_decode_f32(v: f32) -> f32 {
    match v <= 0.04045 {
        true => v / 12.92,
        false => ((v + 0.055) / 1.055).powf(2.4),
    }
}

/// x -> P(x)
pub fn mul_alpha(v: Vec4) -> Vec4 {
    return Vec4::new(v.x * v.w, v.y * v.w, v.z * v.w, v.w);
}

/// P(x) -> x
#[expect(unused)]
pub fn demul_alpha(v: Vec4) -> Vec4 {
    match v.w.is_normal() {
        true => Vec4::new(v.x / v.w, v.y / v.w, v.z / v.w, v.w),
        false => Vec4::ZERO,
    }
}
