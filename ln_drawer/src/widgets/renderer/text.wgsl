#lib_camera

struct TextMaterial {
    color: vec4f,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) @interpolate(flat) kind: u32,
}

@group(1) @binding(0) var<uniform> material: TextMaterial;
@group(2) @binding(0) var atlas: texture_2d<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @location(0) origin: vec2f,
    @location(1) size: vec2f,
    @location(2) uv: vec4f,
    @location(3) kind: u32,
) -> VertexOutput {
    let corner = vec2f(f32(index & 1u), f32((index >> 1u) & 1u));
    let world = origin + size * corner;

    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip_f(world), 0.0, 1.0);
    ret.uv = vec2f(mix(uv.x, uv.z, corner.x), mix(uv.w, uv.y, corner.y));
    ret.kind = kind;
    return ret;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let texel = textureSample(atlas, atlas_sampler, in.uv);

    if in.kind == 0u {
        let alpha = texel.r * material.color.a;
        return vec4f(material.color.rgb * alpha, alpha);
    } else {
        // Color glyphs are premultiplied `P(L)` and ignore the color, only alpha modulates.
        return texel * material.color.a;
    }
}
