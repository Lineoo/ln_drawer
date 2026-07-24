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
    return textureSample(texture, texture_sampler, vertex.uv);
}

@fragment
fn fs_main_debug(vertex: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(texture, texture_sampler, vertex.uv);
    let grid = max(1 - step(vec2f(1. / 512), vertex.uv), step(vec2f(1 - 1. / 512), vertex.uv));
    let grid_float = max(grid.x, grid.y);

    let a = vec4f(color.rgb, 1) * color.a;
    let b = vec4f(vertex.uv, 0.5, 1) * grid_float;
    let c = vec4f(0, 1, 0, 1) * (f32(i32(color.a * 255) % 5) / 5);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return abc;
}
