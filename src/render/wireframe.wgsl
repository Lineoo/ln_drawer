struct Camera {
    size: vec2u,
    center: vec2i,
    center_fract: vec2u,
    zoom: i32,
    zoom_fract: u32,
}

fn camera_convert(world_space: vec2i) -> vec2f {
    let camera_space = world_space - camera.center;
    let camera_scale = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let screen_space = (vec2f(camera_space) - vec2f(camera.center_fract) * vec2f(0x1p-32))
        / vec2f(camera.size) * camera_scale * 2.0;

    return screen_space;
}

struct Rectangle {
    origin: vec2i,
    extend: vec2u,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> rectangle: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let world_space = vec2i(
        rectangle.origin.x + i32(rectangle.extend.x) * i32(index == 1 || index == 2),
        rectangle.origin.y + i32(rectangle.extend.y) * i32(index == 2 || index == 3),
    );

    return vec4f(camera_convert(world_space), 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(1.0, 1.0, 1.0, 1.0);
}