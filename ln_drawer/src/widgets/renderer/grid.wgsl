struct Camera {
    size: vec2u,
    center: vec2i,
    center_fract: vec2u,
    zoom: i32,
    zoom_fract: u32,
}

fn world_to_clip(world_space: vec2i) -> vec2f {
    let camera_space = world_space - camera.center;
    let camera_scale = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let clip_space = (vec2f(camera_space) - vec2f(camera.center_fract) * vec2f(0x1p-32))
        / vec2f(camera.size) * camera_scale * 2.0;

    return clip_space;
}

fn clip_to_world(clip_space: vec2f) -> vec2f {
    let camera_scale = pow(2.0, f32(camera.zoom) + f32(camera.zoom_fract) * 0x1p-32);
    let camera_space = clip_space * vec2f(camera.size) / camera_scale / 2.0 + vec2f(camera.center_fract) * vec2f(0x1p-32);
    let world_space = camera_space + vec2f(camera.center);

    return world_space;
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(1) var<uniform> grid_size: u64;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let screen_space = vec2i(i32(index) / 2, i32(index) % 2) * 2 - vec2i(1);

    var ret: VertexOutput;
    ret.pos = vec4f(vec2f(screen_space), 0.0, 1.0);
    ret.uv = vec2f(vec2i(i32(index) / 2, i32(index) % 2));
    return ret;
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4f {
    let world_space = clip_to_world(in.uv * 2 - vec2f(1));
    let grid_unit = (world_space + vec2f(f32(grid_size) / 2)) / f32(grid_size);
    let grid_mod = grid_unit - floor(grid_unit) - vec2f(0.5);
    return vec4f(vec3f(0.8), 1 - smoothstep(10. / f32(grid_size), 15. / f32(grid_size), length(grid_mod)));
}