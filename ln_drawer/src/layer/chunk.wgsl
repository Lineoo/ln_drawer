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
    let frac = coord - vec2f(base);

    let min_idx = vec2i(0, 0);
    let max_idx = vec2i(textureDimensions(texture)) - vec2i(1, 1);

    let idx_00 = clamp(base + vec2i(0, 0), min_idx, max_idx);
    let idx_10 = clamp(base + vec2i(1, 0), min_idx, max_idx);
    let idx_01 = clamp(base + vec2i(0, 1), min_idx, max_idx);
    let idx_11 = clamp(base + vec2i(1, 1), min_idx, max_idx);

    let c00 = textureLoad(texture, idx_00, 0);
    let c10 = textureLoad(texture, idx_10, 0);
    let c01 = textureLoad(texture, idx_01, 0);
    let c11 = textureLoad(texture, idx_11, 0);

    let p00 = vec4f(c00.rgb * c00.a, c00.a);
    let p10 = vec4f(c10.rgb * c10.a, c10.a);
    let p01 = vec4f(c01.rgb * c01.a, c01.a);
    let p11 = vec4f(c11.rgb * c11.a, c11.a);

    let top = mix(p00, p10, frac.x);
    let bottom = mix(p01, p11, frac.x);
    let result = mix(top, bottom, frac.y);

    return result;
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
