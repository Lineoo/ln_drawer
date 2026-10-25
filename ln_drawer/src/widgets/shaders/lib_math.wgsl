// `n` is vector2, `v` is variance
fn gaussian_2d(n: vec2f, v: f32) -> f32 {
    return FRAC_1_TAU / v * exp(-(n.x * n.x + n.y * n.y) / (2.0 * v));
}

// `n` is scalar, `v` is variance
fn gaussian_1d(n: f32, v: f32) -> f32 {
    return FRAC_1_SQRT_TAU / sqrt(v) * exp(-(n * n) / (2.0 * v));
}

fn erf(x: f32) -> f32 {
    let s  = sign(x);
    let ax = abs(x);
    let t  = 1.0 / (1.0 + 0.3275911 * ax);
    let p  = ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t
              - 0.284496736) * t + 0.254829592) * t;
    return s * (1.0 - p * exp(-ax * ax));
}
