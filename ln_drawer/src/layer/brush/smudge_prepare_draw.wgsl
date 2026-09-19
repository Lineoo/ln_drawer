#lib_constant #lib_rectangle #lib_colorspace #lib_math

struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
    color_ratio: f32,
    sample_radius: f32,
    sample_rate: f32,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;
@group(0) @binding(3) var<storage, read_write> draws_state: array<vec4f>;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

// Evaluate the whole batch's color flow a single time, replacing the disk
// sampling every workgroup used to repeat.
//
// `draws_state[0]` is the carried color: it is read at the start of the batch
// and written back at the end, so it survives across `draw` calls.
// `draws_state[1 + i]` receives the resolved color of dab `i`, letting the
// per-pixel pass just apply masks.
@compute @workgroup_size(1)
fn cs_main() {
    var smudge = draws_state[0];
    var painted = vec4f();

    for (var i = 0u; i < draws_length; i++) {
        let draw = draws_array[i];
        let sample = sample_disk(draw.position, draw.size * draw.sample_radius);

        let foreground = mul_alpha(vec4f(linear_srgb_to_oklab(srgb_gamma_decode(draw.color).xyz), draw.color.a));
        smudge = mix(smudge, foreground, draw.color_ratio);

        let sampled = mul_alpha(vec4f(linear_srgb_to_oklab(demul_alpha(sample).xyz), sample.a));
        let corrected_sampled = painted + sampled * (1.0 - painted.a);
        smudge = mix(smudge, corrected_sampled, draw.sample_rate);

        draws_state[1 + i] = srgb_gamma_encode(vec4f(oklab_to_linear_srgb(demul_alpha(smudge).xyz), smudge.a));
        painted = smudge + painted * (1.0 - smudge.a);
    }

    draws_state[0] = smudge;
}

// sample_radius: alpha-weighted average color inside the disk.
// gaussian_2d use `r = 3σ`
fn sample_disk(center: vec2i, radius: f32) -> vec4f {
    let r = i32(ceil(radius));
    var sum = vec4f();
    var count = 0.0;
    for (var y = -r; y <= r; y++) {
        for (var x = -r; x <= r; x++) {
            let p = center + vec2i(x, y);
            let flag = f32(x * x + y * y) > radius * radius & rectangle_contains(destination, p);
            let k = select(0, gaussian_2d(vec2i(x, y), f32(r * r) / 9.0), flag);
            let c = srgb_gamma_decode(textureLoad(destination_texture, p - destination.coords));
            sum += c * k;
            count += k;
        }
    }
    return sum / max(count, 1e-6);
}
