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

@group(0) @binding(0) var<uniform> viewport: Viewport;
@group(1) @binding(0) var<uniform> rectangle: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let world_space = vec2i(
        rectangle.origin.x + rectangle.extend.x * (i32(index) / 2),
        rectangle.origin.y + rectangle.extend.y * (i32(index) % 2)
    );
    
    let camera_space = vec2f(world_space - viewport.camera);

    // TODO we need to simplify this
    let viewport_range = vec2f(f32(viewport.width), f32(viewport.height));
    let viewport = camera_space / viewport_range * pow(2.0, f32(viewport.zoom)) * 2.0;

    return vec4f(viewport, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(0.8, 0.8, 0.8, 1.0);
}