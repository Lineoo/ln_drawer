struct Viewport {
    width: u32,
    height: u32,
    camera: vec2i,
    zoom: i32,
}

struct Rectangle {
    origin: vec2i,
    extend: vec2i,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) world_space: vec2f,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var<uniform> rectangle: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let world_space = vec2i(
        rectangle.origin.x + rectangle.extend.x * (i32(index) / 2),
        rectangle.origin.y + rectangle.extend.y * (i32(index) % 2)
    );

    let camera_space = vec2f(world_space - viewport.camera);

    // TODO we need to simplify this
    let viewport_range = vec2f(f32(viewport.width), f32(viewport.height));
    let viewport = camera_space / viewport_range * pow(2.0, f32(viewport.zoom)) * 2.0;

    var ret: VertexOutput;
    ret.position = vec4f(viewport, 0.0, 1.0);
    ret.world_space = vec2f(world_space);
    return ret;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    const EDGE = 10;
    let relative = vertex.world_space - vec2f(rectangle.origin);
    let clamped = clamp(relative, vec2f(EDGE), vec2f(rectangle.extend) - vec2f(EDGE));
    let delta = (relative - clamped) / vec2f(EDGE);
    let val = pow(length(delta), 3.0);
    let frame = vec3f(smoothstep(0.4, 0.5, val) - smoothstep(0.5, 1.0, val));
    return vec4f(frame, 1.0);
}