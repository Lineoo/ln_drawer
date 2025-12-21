struct Viewport {
    size: vec2u,
    center: vec2i,
    center_fract: vec2u,
    zoom: i32,
    zoom_fract: u32,
}

fn viewport_convert(world_space: vec2i) -> vec2f {
    let camera_space = world_space - viewport.center;
    let viewport_scale = pow(2.0, f32(viewport.zoom) + f32(viewport.zoom_fract) * 0x1p-32);
    let screen_space = (vec2f(camera_space) - vec2f(viewport.center_fract) * vec2f(0x1p-32))
        / vec2f(viewport.size) * viewport_scale * 2.0;

    return screen_space;
}

struct Rectangle {
    origin: vec2i,
    extend: vec2i,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var<uniform> rectangle: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let world_space = vec2i(
        rectangle.origin.x + rectangle.extend.x * (i32(index) / 2),
        rectangle.origin.y + rectangle.extend.y * (i32(index) % 2)
    );

    var ret: VertexOutput;
    ret.pos = vec4f(viewport_convert(world_space), 0.0, 1.0);
    ret.uv = vec2f(vec2i(i32(index) / 2, 1 ^ i32(index) % 2));
    return ret;
}