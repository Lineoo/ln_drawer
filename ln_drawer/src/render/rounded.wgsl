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

struct RoundedRect {
    origin: vec2i,
    extend: vec2u,
    color: vec4f,
    shrink: f32,
    value: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) relative: vec2f,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> rectangle: RoundedRect;

const vertex_extend: i32 = 10;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let world_space = vec2i(
        rectangle.origin.x + i32(rectangle.extend.x) * (i32(index) / 2),
        rectangle.origin.y + i32(rectangle.extend.y) * (i32(index) % 2)
    );

    // extend 10 pixels to render shadow
    let world_space_extend = world_space + vec2i(
        ((i32(index) / 2) * 2 - 1) * vertex_extend,
        ((i32(index) % 2) * 2 - 1) * vertex_extend,
    );

    var ret: VertexOutput;
    ret.position = vec4f(camera_convert(world_space_extend), 0.0, 1.0);
    ret.relative = vec2f(world_space_extend - rectangle.origin);
    return ret;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    let rect_color = panel(vertex);
    let shadow_color = shadow(vertex);

    return vec4f(
        rect_color.rgb * rect_color.a + shadow_color.rgb * (1.0 - rect_color.a),
        rect_color.a + shadow_color.a * (1.0 - rect_color.a)
    );
    // return vec4f(step(distance, rectangle.value)) * rectangle.color;
    // return vec4f(normalize(vec3f(0.5, 0.5, distance)), fract(distance)) * rectangle.color;
}

fn panel(vertex: VertexOutput) -> vec4f {
    let point = abs(vertex.relative - vec2f(rectangle.extend) / 2.0);
    let corner = vec2f(rectangle.extend) / 2.0 - rectangle.shrink;

    let delta = point - corner;
    let distance = length(max(delta, vec2f(0.0))) + min(max(delta.x, delta.y), 0.0);

    let diff = rectangle.value - distance;
    let width = fwidth(diff) * 0.5;

    return vec4f(rectangle.color.rgb, rectangle.color.a * smoothstep(-width, width, diff));
}

fn shadow(vertex: VertexOutput) -> vec4f {
    const shadow_offset = vec2f(0.0, -4.0);
    const shadow_blur = 10.0;
    const shadow_color = vec4f(0.0, 0.0, 0.0, 0.5);

    let shadow_pos = vertex.relative - shadow_offset;

    let point = abs(shadow_pos - vec2f(rectangle.extend) / 2.0);
    let corner = vec2f(rectangle.extend) / 2.0 - rectangle.shrink;

    let delta = point - corner;
    let distance = length(max(delta, vec2f(0.0))) + min(max(delta.x, delta.y), 0.0);

    let diff = rectangle.value - distance;

    return vec4f(shadow_color.rgb, shadow_color.a * smoothstep(-shadow_blur, shadow_blur, diff));
}