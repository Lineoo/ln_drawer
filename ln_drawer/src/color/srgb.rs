pub fn srgb_gamma_encode(v: f32) -> f32 {
    match v < 0.0031308 {
        true => v * 12.92,
        false => 1.055 * v.powf(1.0 / 2.4) - 0.055,
    }
}

pub fn srgb_gamma_decode(v: f32) -> f32 {
    match v <= 0.04045 {
        true => v / 12.92,
        false => ((v + 0.055) / 1.055).powf(2.4),
    }
}
