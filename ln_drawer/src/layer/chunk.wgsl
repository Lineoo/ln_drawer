struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(1) @binding(0) var texture_sampler: sampler;

@group(2) @binding(0) var texture: texture_2d<f32>;
@group(2) @binding(1) var<uniform> chunk: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let texture_uv = vec2u(index / 2, index % 2);
    let world_space = chunk.coords + vec2i(chunk.size * texture_uv);

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(world_space), 0.0, 1.0);
    ret.uv = vec2f(texture_uv);
    return ret;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    let dims = vec2f(textureDimensions(texture));
    
    let coord = vertex.uv * dims - 0.5;
    let base = vec2i(floor(coord));
    let frac = fract(coord);

    let min_idx = vec2i(0, 0);
    let max_idx = vec2i(dims) - vec2i(1, 1);

    let idx_00 = clamp(base + vec2i(0, 0), min_idx, max_idx);
    let idx_10 = clamp(base + vec2i(1, 0), min_idx, max_idx);
    let idx_01 = clamp(base + vec2i(0, 1), min_idx, max_idx);
    let idx_11 = clamp(base + vec2i(1, 1), min_idx, max_idx);

    let c00_ump = srgb_to_linear(textureLoad(texture, idx_00, 0));
    let c10_ump = srgb_to_linear(textureLoad(texture, idx_10, 0));
    let c01_ump = srgb_to_linear(textureLoad(texture, idx_01, 0));
    let c11_ump = srgb_to_linear(textureLoad(texture, idx_11, 0));

    let c00 = vec4f(c00_ump.rgb, 1) * c00_ump.a;
    let c10 = vec4f(c10_ump.rgb, 1) * c10_ump.a;
    let c01 = vec4f(c01_ump.rgb, 1) * c01_ump.a;
    let c11 = vec4f(c11_ump.rgb, 1) * c11_ump.a;

    let result = mix(mix(c00, c10, frac.x), mix(c01, c11, frac.x), frac.y);
    let result_srgb_ump = linear_to_srgb(alpha_premultiplied_invert(result));
    return vec4f(result_srgb_ump.rgb, 1) * result_srgb_ump.a;
}

@fragment
fn fs_fast(vertex: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(texture, texture_sampler, vertex.uv);
    return vec4f(color.rgb, 1) * color.a;
}

@fragment
fn fs_debug0(vertex: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(texture, texture_sampler, vertex.uv);
    let grid = max(1 - step(vec2f(5. / 512), vertex.uv), step(vec2f(1 - 5. / 512), vertex.uv));
    let grid_float = max(grid.x, grid.y);

    let a = vec4f(color.rgb, 1) * color.a;
    let b = vec4f(vertex.uv, 0.5, 0.5) * (grid_float * 0.8 + 0.2);
    let c = vec4f(0, 1, 0, 1) * (f32(i32(color.a * 255) % 5) / 5);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return abc;
}

@fragment
fn fs_debug1(vertex: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(texture, texture_sampler, vertex.uv);
    let grid = max(1 - step(vec2f(5. / 512), vertex.uv), step(vec2f(1 - 5. / 512), vertex.uv));
    let grid_float = max(grid.x, grid.y);

    let a = vec4f(color.rgb, 1) * color.a;
    let b = vec4f(vertex.uv, 0, 0.8) * (grid_float * 0.8 + 0.2);
    let c = vec4f(0, 1, 0, 1) * (f32(i32(color.a * 255) % 5) / 5);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return abc;
}
