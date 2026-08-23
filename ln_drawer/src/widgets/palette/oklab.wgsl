// include! colorspace constant

struct PaletteOklch {
    oklab: vec4f,
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
};

@group(1) @binding(1) var<uniform> palette: PaletteOklch;

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
    return cmp + (1 - cmp.a) * chb;
}

fn color_main_palette(uv: vec2f) -> vec4f {
    let within = step(0, uv.x) * (1 - step(1, uv.x))
        * step(0, uv.y) * (1 - step(1, uv.y));
    
    let color = oklab_to_linear_srgb(vec3f(palette.oklab.x, uv.xy * 0.8 - 0.4));
    return select(vec4f(), vec4f(color, 1) * within, all(color <= vec3f(1) & color >= vec3f(0)));
}

fn color_hue_band(radius: f32, angle: f32) -> vec4f {
    let alpha = hue_alpha(radius);

    let color = oklab_to_linear_srgb(vec3f(fract(angle / TAU), palette.oklab.yz));
    return select(vec4f(), vec4f(color, 1) * alpha, all(color <= vec3f(1) & color >= vec3f(0)));
}

fn hue_alpha(radius: f32) -> f32 {
    let r_width = max(1e-6, fwidth(radius) * 0.5);
    return min(
        smoothstep(0.5 - r_width, 0.5 + r_width, radius + palette.band_width),
        smoothstep(0.5 + r_width, 0.5 - r_width, radius),
    );
}