struct PaletteHsl {
    band_width: f32,
    hue: f32,
    saturation: f32,
    lightness: f32,
};

@group(1) @binding(1) var<uniform> palette: PaletteHsl;

const TAU: f32 = 6.28318530717958647692528676655900577;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4f {
    let sq_size = (0.5 - palette.band_width) * sqrt(2);
    let sq_uv = (in.uv - 0.5) / sq_size + 0.5;
    if all(sq_uv < vec2f(1) & sq_uv > vec2f(0)) {
        return vec4f(hsl_to_rgb(vec3f(palette.hue, sq_uv.x, sq_uv.y)), 1.0);
    }

    let delta = in.uv - vec2f(0.5);
    let dist = length(delta);
    if dist < 0.5 && dist > 0.5 - palette.band_width {
        let hue = fract(atan2(delta.y, delta.x) / TAU + 1);
        return vec4f(hsl_to_rgb(vec3f(hue, palette.saturation, palette.lightness)), 1.0);
    }

    return vec4f();
}

fn hsl_to_rgb(hsl: vec3f) -> vec3f {
    return hsl.z + hsl.y * (hue_to_rgb(hsl.x) - 0.5) * (1.0 - abs(2.0 * hsl.z - 1.0));
}

fn hue_to_rgb(h: f32) -> vec3f {
    return clamp(abs(((h * 6.0 + vec3f(0.0, 4.0, 2.0)) % 6.0) - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
}