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

@group(0) @binding(0) var<uniform> color: vec4f;

@group(1) @binding(0) var<uniform> viewport: Viewport;

@vertex
fn vs_main(@location(0) world_space: vec2i) -> @builtin(position) vec4f {
    return vec4f(viewport_convert(world_space), 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}