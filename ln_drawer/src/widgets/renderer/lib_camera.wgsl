struct Camera {
    size: vec2u,
    center: vec2i,
    center_fract: vec2u,
    zoom: i32,
    zoom_fract: u32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

fn world_to_clip(world_space: vec2i) -> vec2f {
    let camera_space = vec2f(world_space - camera.center) - vec2f(camera.center_fract) * vec2f(0x1p-32);
    let camera_zoom = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let camera_scaled_space = camera_space * camera_zoom;
    let clip_space = round(camera_scaled_space) / vec2f(camera.size) * 2.0;

    return clip_space;
}

fn clip_to_world(clip_space: vec2f) -> vec2i {
    let camera_zoom = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let camera_scaled_space = clip_space * vec2f(camera.size) / 2.0;
    let camera_space = camera_scaled_space / camera_zoom;
    let world_space = vec2i(round(camera_space + vec2f(camera.center_fract) * vec2f(0x1p-32))) + camera.center;

    return world_space;
}

fn clip_to_world_vec2f(clip_space: vec2f) -> vec2f {
    let camera_scale = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let camera_space = clip_space * vec2f(camera.size) / camera_scale / 2.0 + vec2f(camera.center_fract) * vec2f(0x1p-32);
    return camera_space + vec2f(camera.center);
}