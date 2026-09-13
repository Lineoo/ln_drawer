// `n` is vector2, `v` is variance
fn gaussian_2d(n: vec2i, v: f32) -> f32 {
    return FRAC_1_TAU / v * exp(-f32(n.x * n.x + n.y * n.y) / (2.0 * v));
}

// `n` is scalar, `v` is variance
fn gaussian_1d(n: i32, v: f32) -> f32 {
    return FRAC_1_SQRT_TAU / sqrt(v) * exp(-f32(n * n) / (2.0 * v));
}