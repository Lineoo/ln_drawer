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
@group(0) @binding(3) var<storage, read_write> draws_state: vec4f;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, #read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, #write>;
@group(2) @binding(1) var<uniform> swap: Rectangle;

// Per-dab color sampled from the canvas, shared by every pixel in the
// workgroup since it only depends on the dab position.
var<workgroup> samples: array<vec4f, 256>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u, @builtin(local_invocation_id) lid: vec3u) {
    let start = max(max(dispatch.coords, destination.coords), swap.coords);
    let position = start + vec2i(id.xy);

    let tid = lid.x + lid.y * 16u;
    if tid < draws_length {
        let draw = draws_array[tid];
        samples[tid] = sample_disk(draw.position, draw.size * draw.sample_radius);
    }

    workgroupBarrier();

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position)
        && rectangle_contains(swap, position);
    if !validated { return; }

    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    let dst_ump = textureLoad(destination_texture, dst_coords);
    var dst = vec4f(linear_srgb_to_oklab(srgb_to_linear(dst_ump).xyz) * dst_ump.a, dst_ump.a);

    // sample_rate: how fast the carried color is replaced by the new sample.
    var smudge = draws_state;
    var painted = vec4f();
    for (var i = 0u; i < draws_length; i++) {
        let draw = draws_array[i];
        let sampled = samples[i];

        // Approximately correct the in-batch sampled color
        let corrected_sampled = painted + sampled * (1.0 - painted.a);
        smudge = select(corrected_sampled, mix(smudge, corrected_sampled, draw.sample_rate), i > 0u);

        // color_ratio: foreground color share of the mixed result.
        let foreground = vec4f(linear_srgb_to_oklab(srgb_to_linear(draw.color).xyz), 1.0) * draw.color.a;
        let paint = mix(smudge, foreground, draw.color_ratio);

        let dist = length(vec2f(draw.position - position) - vec2f(0.5) + vec2f(draw.position_fract) * 0x1p-32);
        let mask = smoothstep((1.0 + draw.softness) * draw.size + 0.5, (1.0 - draw.softness) * draw.size + 0.5, dist);

        let src = paint * draw.flow * mask;
        dst = src + dst * (1.0 - src.a);
        painted = src + painted * (1.0 - src.a);
    }

    let dst_ump_out = alpha_premultiplied_invert(dst);
    let dst_out = linear_to_srgb(vec4f(oklab_to_linear_srgb(dst_ump_out.xyz), dst_ump_out.a));

    textureStore(swap_texture, swp_coords, dst_out);
    draws_state = smudge;
}

// sample_radius: alpha-weighted average color inside the disk.
// gaussian_2d use `r = 3σ`
fn sample_disk(center: vec2i, radius: f32) -> vec4f {
    let r = i32(ceil(radius));
    var sum = vec4f();
    var count = 0.0;
    for (var y = -r; y <= r; y++) {
        for (var x = -r; x <= r; x++) {
            if f32(x * x + y * y) > radius * radius { continue; }
            let p = center + vec2i(x, y);
            if !rectangle_contains(destination, p) { continue; }
            let c = textureLoad(destination_texture, p - destination.coords);
            let oklab = linear_srgb_to_oklab(srgb_to_linear(c).xyz);
            let priority = gaussian_2d(vec2i(x, y), f32(r * r) / 9.0);
            sum += vec4f(oklab * c.a, c.a) * priority;
            count += priority;
        }
    }
    return sum / max(count, 1e-6);
}
