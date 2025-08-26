struct Viewport {
    width: u32,
    height: u32,
    camera: vec2i,
    zoom: i32,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@group(1) @binding(0) var<uniform> viewport: Viewport;

@vertex
fn vs_main(@location(0) world_space: vec2i, @location(1) uv: vec2f) -> VertexOutput {
    var output: VertexOutput;

    output.pos = vec4f(
        2.0 * vec2f(world_space - viewport.camera) / vec2f(
            f32(viewport.width),
            f32(viewport.height)
        ) * pow(2.0, f32(viewport.zoom)),
        0.0, 
        1.0
    );
    output.uv = uv;

    return output;
}

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    return textureSample(texture, texture_sampler, uv);
}
