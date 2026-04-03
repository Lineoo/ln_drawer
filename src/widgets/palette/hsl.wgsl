struct PaletteHsl {
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
    hue: f32,
    saturation: f32,
    lightness: f32,
};

@group(1) @binding(1) var<uniform> palette: PaletteHsl;

const TAU: f32 = 6.28318530717958647692528676655900577;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) @interpolate(perspective, sample) uv: vec2f,
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4f {
    let delta = in.uv - vec2f(0.5);
    let radius = length(delta);
    let angle = atan2(delta.y, delta.x);

    let sq_size = (0.5 - palette.band_width) * sqrt(2.0);
    let sq_uv = (in.uv - 0.5) / sq_size + 0.5;

    let cmp = color_main_palette(sq_uv);
    let chb = color_hue_band(radius, angle);
    let bg = mix(cmp, chb, chb.a);

    let cmk = color_main_knob(sq_uv);
    let chk = color_hue_knob(radius, angle);
    let kb = mix(cmk, chk, chk.a);

    return mix(bg, kb, kb.a);
}

fn color_main_palette(uv: vec2f) -> vec4f {
    if uv.x > 0 && uv.x < 1 && uv.y > 0 && uv.y < 1 {
        return vec4f(hsl_to_rgb(palette.hue, uv.x, uv.y), 1.0);
    }

    return vec4f();
}

fn color_main_knob(uv: vec2f) -> vec4f {
    let e = distance(uv, vec2f(palette.saturation, palette.lightness));
    if e < palette.main_knob_size {
        return vec4f(hsl_to_rgb(palette.hue, palette.saturation, palette.lightness), 1.0);
    } else if e < palette.main_knob_size + 0.001 {
        return vec4f(1, 1, 1, 1);
    } else if e < palette.main_knob_size + 0.002 {
        return vec4f(0, 0, 0, 1);
    }

    return vec4f();
}

fn color_hue_band(radius: f32, angle: f32) -> vec4f {
    if radius > 0.5 || radius < 0.5 - palette.band_width {
        return vec4f();
    }

    let hue = fract(angle / TAU + 1);
    return vec4f(hsl_to_rgb(hue, palette.saturation, palette.lightness), 1.0);
}

fn color_hue_knob(radius: f32, angle: f32) -> vec4f {
    if radius > 0.5 || radius < 0.5 - palette.band_width {
        return vec4f();
    }

    let hue = fract(angle / TAU);
    let diff = abs(palette.hue - hue);
    let e = min(diff, 1 - diff);
    if e < palette.hue_knob_size {
        return vec4f(hsl_to_rgb(palette.hue, palette.saturation, palette.lightness), 1.0);
    } else if e < palette.hue_knob_size + 0.001 {
        return vec4f(1, 1, 1, 1);
    } else if e < palette.hue_knob_size + 0.002 {
        return vec4f(0, 0, 0, 1);
    }

    return vec4f();
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3f {
    return l + s * (hue_to_rgb(h) - 0.5) * (1.0 - abs(2.0 * l - 1.0));
}

fn hue_to_rgb(h: f32) -> vec3f {
    return clamp(abs(((h * 6.0 + vec3f(0.0, 4.0, 2.0)) % 6.0) - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
}