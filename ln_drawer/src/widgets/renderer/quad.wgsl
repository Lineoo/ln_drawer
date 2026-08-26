// include! camera

struct Quad {
    origin: vec2i,
    extend: vec2u,
    edge: vec2u,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(1) @binding(0) var<uniform> quad: Quad;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let pose = vec2i(i32(index / 2), i32(index % 2));
    let world_space = quad.origin + vec2i(quad.extend) * pose + vec2i(quad.edge) * (2 * pose - 1);

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(world_space), 0.0, 1.0);
    ret.uv = vec2f(world_space - quad.origin) / vec2f(quad.extend);
    return ret;
}
