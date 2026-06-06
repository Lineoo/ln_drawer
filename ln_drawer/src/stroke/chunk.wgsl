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

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var<uniform> rectangle: Rectangle;
@group(1) @binding(1) var texture: texture_2d<f32>;
@group(1) @binding(2) var texture_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let world_space = vec2i(
        rectangle.origin.x + i32(rectangle.extend.x) * (i32(index) / 2),
        rectangle.origin.y + i32(rectangle.extend.y) * (i32(index) % 2)
    );

    var ret: VertexOutput;
    ret.pos = vec4f(camera_convert(world_space), 0.0, 1.0);
    ret.uv = vec2f(vec2i(i32(index) / 2, i32(index) % 2));
    return ret;
}


@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    return textureSample(texture, texture_sampler, vertex.uv);
}

@fragment
fn fs_main_debug(vertex: VertexOutput) -> @location(0) vec4f {
    let intensity = log2(f32(rectangle.extend.x) / 512) / 8.0;
    let color = textureSample(texture, texture_sampler, vertex.uv);
    let a = vec4f(color.rgb, 1) * color.a;
    let b = vec4f(1, 0, 0, 1) * intensity;
    let c = vec4f(0, 1, 0, 1) * (f32(i32(color.a * 255) % 5) / 5);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return abc;
}