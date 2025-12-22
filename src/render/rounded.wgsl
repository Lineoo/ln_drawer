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
    extend: vec2u,
    color: vec4f,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) world_space: vec2f,
}

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var<uniform> rectangle: Rectangle;

const edge = 10;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let world_space = vec2i(
        rectangle.origin.x + i32(rectangle.extend.x) * (i32(index) / 2),
        rectangle.origin.y + i32(rectangle.extend.y) * (i32(index) % 2)
    );

    // extend 10 pixels to render shadow
    let world_space_extend = world_space + vec2i(
        ((i32(index) / 2) * 2 - 1) * edge,
        ((i32(index) % 2) * 2 - 1) * edge,
    );

    var ret: VertexOutput;
    ret.position = vec4f(viewport_convert(world_space_extend), 0.0, 1.0);
    ret.world_space = vec2f(world_space_extend);
    return ret;
}

const shrink = 5.0;
const step_value = 5.0;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    let relative = vertex.world_space - vec2f(rectangle.origin);
    
    // use SDF to calculate rectangle
    let point = abs(relative - vec2f(rectangle.extend) / 2.0);
    let corner = vec2f(rectangle.extend) / 2.0 - shrink;

    let delta = point - corner;
    let distance = length(max(delta, vec2f(0.0))) + min(max(delta.x, delta.y), 0.0);

    return vec4f(step(distance, step_value)) * rectangle.color;
}