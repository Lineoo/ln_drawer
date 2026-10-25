#lib_camera #lib_colorspace

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(1) @binding(0) var texture_sampler: sampler;
@group(1) @binding(1) var<uniform> background_color: vec4f;

@group(2) @binding(0) var texture: texture_2d<f32>;
@group(2) @binding(1) var<uniform> chunk: Rectangle;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let texture_uv = vec2u(index / 2, index % 2);
    let world_space = chunk.coords + vec2i(chunk.size * texture_uv);

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(world_space), 0.0, 1.0);
    ret.uv = vec2f(texture_uv);
    return ret;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    let color = mul_alpha(srgb_gamma_encode(demul_alpha(textureSample(texture, texture_sampler, vertex.uv))));
    return srgb_gamma_decode(color + background_color * (1 - color.a));
}

@fragment
fn fs_debug(vertex: VertexOutput) -> @location(0) vec4f {
    let color = mul_alpha(srgb_gamma_encode(demul_alpha(textureSample(texture, texture_sampler, vertex.uv))));
    let grid_pixel = floor(vertex.uv * 512.);
    let grid = (grid_pixel % 2 * 0.5 + 0.5)
        * max(smoothstep(vec2f(16), vec2f(0), grid_pixel), smoothstep(vec2f(496), vec2f(512), grid_pixel));

    let a = vec4f(vertex.uv * 0.5 + 0.25, 0.5, max(grid.x, grid.y));
    let b = color;
    let c = vec4f(1 - color.a, color.a, 0, 1) * fract(color.a * 8);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return srgb_gamma_decode(abc + background_color * (1 - abc.a));
}