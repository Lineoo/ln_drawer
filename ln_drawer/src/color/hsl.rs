use glam::{Vec3, Vec4};

/// L_hsl -> S(L_srgb)
pub fn hsl_to_srgb(hsla: Vec4) -> Vec4 {
    let rgb = ((((hsla.x * 6.0 + Vec3::new(0.0, 4.0, 2.0)) % 6.0) - 3.0).abs() - 1.0)
        .clamp(Vec3::splat(0.0), Vec3::splat(1.0));
    return (hsla.z + hsla.y * (rgb - 0.5) * (1.0 - (2.0 * hsla.z - 1.0).abs())).extend(hsla.w);
}

/// S(L_srgb) -> L_hsl
pub fn srgb_to_hsl(rgba: Vec4) -> Vec4 {
    let max = (rgba.x).max(rgba.y).max(rgba.z);
    let min = (rgba.x).min(rgba.y).min(rgba.z);

    let c = max - min;
    let l = (max + min) * 0.5;

    if c == 0.0 {
        return Vec4::new(0.0, 0.0, l, rgba.w);
    }

    let s = c / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == rgba.x {
        ((rgba.y - rgba.z) / c).rem_euclid(6.0) / 6.0
    } else if max == rgba.y {
        ((rgba.z - rgba.x) / c + 2.0) / 6.0
    } else {
        ((rgba.x - rgba.y) / c + 4.0) / 6.0
    };

    Vec4::new(h, s, l, rgba.w)
}
