struct Camera {
    center: vec2i,
    center_fract: vec2u,
    transform: mat2x2f,
    inverse: mat2x2f,
    size: vec2f,
    _pad: vec2f,
}

@group(0) @binding(0) var<uniform> camera: Camera;

fn world_to_clip(world_space: vec2i) -> vec2f {
    return camera.transform * (vec2f(world_space - camera.center) - vec2f(camera.center_fract) * vec2f(0x1p-32));
}

fn world_to_clip_f(world_space: vec2f) -> vec2f {
    return camera.transform * (world_space - vec2f(camera.center) - vec2f(camera.center_fract) * vec2f(0x1p-32));
}

fn world_to_clip_relative(world_space: vec2f) -> vec2f {
    return camera.transform * world_space;
}

fn clip_to_world(clip_space: vec2f) -> vec2i {
    return vec2i(round(camera.inverse * clip_space + vec2f(camera.center_fract) * vec2f(0x1p-32))) + camera.center;
}

fn snap_to_pixel_min(clip_space: vec2f) -> vec2f {
    let pixel = (clip_space * 0.5 + 0.5) * camera.size;
    return (floor(pixel) / camera.size) * 2.0 - 1.0;
}

fn snap_to_pixel_max(clip_space: vec2f) -> vec2f {
    let pixel = (clip_space * 0.5 + 0.5) * camera.size;
    return (ceil(pixel) / camera.size) * 2.0 - 1.0;
}
