struct Viewport {
    width: u32,
    height: u32,
    camera: vec2i,
    zoom: i32,
}

struct Rectangle {
    origin: vec2i,
    extend: vec2i,
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
        rectangle.origin.x + rectangle.extend.x * (i32(index) / 2),
        rectangle.origin.y + rectangle.extend.y * (i32(index) % 2)
    );

    // extend 10 pixels to render shadow
    let world_space_extend = world_space + vec2i(
        ((i32(index) / 2) * 2 - 1) * edge,
        ((i32(index) % 2) * 2 - 1) * edge,
    );

    let camera_space = vec2f(world_space_extend - viewport.camera);

    let viewport_range = vec2f(f32(viewport.width), f32(viewport.height));
    let viewport = camera_space / viewport_range * pow(2.0, f32(viewport.zoom)) * 2.0;

    // TODO using world_space directly feels weird and will cause precision loss
    var ret: VertexOutput;
    ret.position = vec4f(viewport, 0.0, 1.0);
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