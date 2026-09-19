#lib_camera #lib_colorspace

struct Quad {
    origin: vec2i,
    extend: vec2u,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(1) @binding(0) var<uniform> quad: Quad;
@group(1) @binding(1) var texture: texture_2d<f32>;
@group(1) @binding(2) var texture_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let pose = vec2i(i32(index / 2), i32(index % 2));
    let world_space = quad.origin + vec2i(quad.extend) * pose;

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(world_space), 0.0, 1.0);
    ret.uv = vec2f(pose);
    return ret;
}

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let color = mul_alpha(srgb_gamma_encode(demul_alpha(textureSample(texture, texture_sampler, uv))));
    return srgb_gamma_decode(color + vec4f(1) * (1 - color.a));
}
