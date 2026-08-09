// #include constant rectangle

struct Draw {
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    sigma: f32,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var<uniform> swap: Rectangle;

var<workgroup> intermediate: array<array<vec4f, 32>, 32>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u, @builtin(local_invocation_id) lid: vec3u) {
    let start = max(max(dispatch.coords, destination.coords), swap.coords);
    let position = start + vec2i(id.xy);

    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    intermediate[lid.x][lid.y] = textureLoad(destination_texture, dst_coords + vec2i(-8, -8));
    intermediate[lid.x][lid.y + 16] = textureLoad(destination_texture, dst_coords + vec2i(-8, 8));
    intermediate[lid.x + 16][lid.y] = textureLoad(destination_texture, dst_coords + vec2i(8, -8));
    intermediate[lid.x + 16][lid.y + 16] = textureLoad(destination_texture, dst_coords + vec2i(8, 8));

    // barrier BEFORE validation
    workgroupBarrier();

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(swap, position);
    if !validated { return; }

    var variance = 1e-6;
    for (var i = 0u; i < draws_length; i++) {
        let draw = draws_array[i];
        let dist = length(vec2f(draw.position - position) - vec2f(0.5) + vec2f(draw.position_fract) * 0x1p-32);
        let mask = smoothstep((1.0 + draw.softness) * draw.size + 0.5, (1.0 - draw.softness) * draw.size + 0.5, dist);

        let masked_sigma = draw.sigma * mask;
        variance += masked_sigma * masked_sigma;
    }

    var k_sum = 0.0;
    var dst = vec4f();
    let radius = clamp(sqrt(variance) * 3.0, 1.0, 8.0);
    for (var x = i32(round(-radius)); x <= i32(round(radius)); x++) {
        for (var y = i32(round(-radius)); y <= i32(round(radius)); y++) {
            let cnv = position + vec2i(x, y);
            let cnv_coords = vec2i(lid.xy) + vec2i(8, 8) + vec2i(x, y);

            if !rectangle_contains(destination, cnv) { continue; }

            let k = gaussian_2d(vec2i(x, y), variance);
            let dst_ump = intermediate[cnv_coords.x][cnv_coords.y];

            k_sum += k;
            dst += vec4f(dst_ump.rgb, 1) * dst_ump.a * k;
        } 
    }

    // Normalize before storage
    dst /= max(k_sum, 1e-6);

    textureStore(swap_texture, swp_coords, select(vec4f(dst.rgb / dst.a, dst.a), vec4f(), dst.a < 1e-6));
}

fn gaussian_2d(n: vec2i, v: f32) -> f32 {
    return FRAC_1_TAU / v * exp(-f32(n.x * n.x + n.y * n.y) / (2.0 * v));
}

fn gaussian_1d(n: i32, v: f32) -> f32 {
    return FRAC_1_SQRT_TAU / sqrt(v) * exp(-f32(n * n) / (2.0 * v));
}