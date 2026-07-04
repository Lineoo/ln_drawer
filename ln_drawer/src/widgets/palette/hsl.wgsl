// include! colorspace

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
const WHITE: vec4f = vec4f(1);
const BLACK: vec4f = vec4f(vec3f(0), 1);

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
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
    let bg = cmp + (1 - cmp.a) * chb;

    let cmk = color_main_knob(sq_uv);
    let chk = color_hue_knob(radius, angle);
    let kb = cmk + (1 - cmk.a) * chk;

    return kb + (1 - kb.a) * bg;
}

fn color_main_palette(uv: vec2f) -> vec4f {
    let within = step(0, uv.x) * (1 - step(1, uv.x))
        * step(0, uv.y) * (1 - step(1, uv.y));
    
    let color = srgb_to_linear(vec4f(hsl_to_rgb(palette.hue, uv.x, uv.y), 1));
    return color * within;
}

fn color_hue_band(radius: f32, angle: f32) -> vec4f {
    let alpha = hue_alpha(radius);

    let color = srgb_to_linear(vec4f(hsl_to_rgb(fract(angle / TAU + 1), palette.saturation, palette.lightness), 1));
    return color * alpha;
}

fn color_main_knob(uv: vec2f) -> vec4f {
    let diff = distance(uv, vec2f(palette.saturation, palette.lightness)) - palette.main_knob_size;
    let width = fwidth(diff) * 0.5;
    if diff < 0.010 {
        let factor = smoothstep(-width, width, diff - 0.008);
        let color = srgb_to_linear(vec4f(hsl_to_rgb(palette.hue, palette.saturation, palette.lightness), 1));
        return mix(color, WHITE, factor);
    } else if diff < 0.014 {
        let factor = smoothstep(-width, width, diff - 0.012);
        return mix(WHITE, BLACK, factor);
    } else {
        let factor = smoothstep(-width, width, diff - 0.016);
        return mix(BLACK, vec4f(), factor);
    }
}

fn color_hue_knob(radius: f32, angle: f32) -> vec4f {
    let alpha = hue_alpha(radius);

    let hue = fract(angle / TAU);
    let diff_d = abs(palette.hue - hue);
    let diff = min(diff_d, 1 - diff_d) - palette.hue_knob_size;
    let width = fwidth(diff) * 0.5;

    if diff < 0.0010 {
        let factor = smoothstep(-width, width, diff - 0.0005);
        let color = srgb_to_linear(vec4f(hsl_to_rgb(palette.hue, palette.saturation, palette.lightness), 1));
        return mix(color, WHITE, factor) * alpha;
    } else if diff < 0.0020 {
        let factor = smoothstep(-width, width, diff - 0.0015);
        return mix(WHITE, BLACK, factor) * alpha;
    } else {
        let factor = smoothstep(-width, width, diff - 0.0025);
        return mix(BLACK, vec4f(), factor) * alpha;
    }
}

fn hue_alpha(radius: f32) -> f32 {
    let r_width = max(1e-6, fwidth(radius) * 0.5);
    return min(
        smoothstep(0.5 - r_width, 0.5 + r_width, radius + palette.band_width),
        smoothstep(0.5 + r_width, 0.5 - r_width, radius),
    );
}