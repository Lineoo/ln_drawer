#lib_camera

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
    let extend = vec2f(quad.extend);
    let uv = vec2f(pose) + vec2f(2 * pose - 1) * vec2f(quad.edge) / extend;

    let left_down = snap_to_pixel_min(world_to_clip(quad.origin));
    let right_up = snap_to_pixel_max(world_to_clip(quad.origin + vec2i(quad.extend)));

    var ret: VertexOutput;
    ret.pos = vec4f(left_down + (right_up - left_down) * uv, 0.0, 1.0);
    ret.uv = uv;
    return ret;
}
