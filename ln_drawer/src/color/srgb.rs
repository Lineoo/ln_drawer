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

/// `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`
pub fn hex_to_srgb(hex: &str) -> Vec4 {
    let hex = hex.trim().strip_prefix('#').unwrap_or(hex);
    assert!(
        matches!(hex.len(), 3 | 4 | 6 | 8),
        "invalid hex color: {hex}"
    );

    let digits: Vec<u8> = hex
        .bytes()
        .map(|b| (b as char).to_digit(16).expect("invalid hex digit") as u8)
        .collect();

    let channel = |high: u8, low: u8| (high * 16 + low) as f32 / 255.0;
    match digits.as_slice() {
        [r, g, b] => Vec4::new(channel(*r, *r), channel(*g, *g), channel(*b, *b), 1.0),
        [r, g, b, a] => Vec4::new(
            channel(*r, *r),
            channel(*g, *g),
            channel(*b, *b),
            channel(*a, *a),
        ),
        [r0, r1, g0, g1, b0, b1] => {
            Vec4::new(channel(*r0, *r1), channel(*g0, *g1), channel(*b0, *b1), 1.0)
        }
        [r0, r1, g0, g1, b0, b1, a0, a1] => Vec4::new(
            channel(*r0, *r1),
            channel(*g0, *g1),
            channel(*b0, *b1),
            channel(*a0, *a1),
        ),
        _ => unreachable!(),
    }
}

/// x -> P(x)
pub fn mul_alpha(v: Vec4) -> Vec4 {
    return Vec4::new(v.x * v.w, v.y * v.w, v.z * v.w, v.w);
}

/// P(x) -> x
pub fn demul_alpha(v: Vec4) -> Vec4 {
    match v.w.is_normal() {
        true => Vec4::new(v.x / v.w, v.y / v.w, v.z / v.w, v.w),
        false => Vec4::ZERO,
    }
}
