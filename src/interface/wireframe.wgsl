struct Viewport {
    width: u32,
    height: u32,
    camera: vec2i,
    zoom: i32,
}

@group(0) @binding(0) var<uniform> color: vec4f;

@group(1) @binding(0) var<uniform> viewport: Viewport;

@vertex
fn vs_main(@location(0) world_space: vec2i) -> @builtin(position) vec4f {
    return vec4f(
        2.0 * vec2f(world_space - viewport.camera) / vec2f(
            f32(viewport.width),
            f32(viewport.height)
        ) * pow(2.0, f32(viewport.zoom)),
        0.0, 
        1.0
    );
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}