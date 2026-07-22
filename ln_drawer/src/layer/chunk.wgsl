struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(1) @binding(0) var texture_sampler: sampler;

@group(2) @binding(0) var<uniform> chunk_key: vec3i;
@group(2) @binding(1) var texture: texture_2d<f32>;

const texture_base_size: i32 = 512;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let texel_size = texture_base_size * i32(exp2(f32(chunk_key.z)));
    let world_origin = chunk_key.xy * texel_size;

    let world_space = vec2i(
        world_origin.x + texel_size * (i32(index) / 2),
        world_origin.y + texel_size * (i32(index) % 2)
    );

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(world_space), 0.0, 1.0);
    ret.uv = vec2f(vec2i(i32(index) / 2, i32(index) % 2));
    return ret;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    return textureSample(texture, texture_sampler, vertex.uv);
}

@fragment
fn fs_main_debug(vertex: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(texture, texture_sampler, vertex.uv);
    let grid = max(1 - step(vec2f(1. / 512), vertex.uv), step(vec2f(1 - 1. / 512), vertex.uv));
    let grid_float = max(grid.x, grid.y);

    let a = vec4f(color.rgb, 1) * color.a;
    let b = vec4f(vertex.uv, 0.5, 1) * grid_float;
    let c = vec4f(0, 1, 0, 1) * (f32(i32(color.a * 255) % 5) / 5);

    let ab = a * (1 - b.a) + b;
    let abc = ab * (1 - c.a) + c;
    return abc;
}
