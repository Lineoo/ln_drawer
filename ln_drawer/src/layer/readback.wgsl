#lib_rectangle #lib_colorspace

@group(0) @binding(0) var<uniform> sample: vec2i;
@group(0) @binding(1) var<storage, read_write> result: vec4f;

@group(1) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let position = sample + vec2i(id.xy);
    let src_coords = position - source.coords;

    var color = srgb_gamma_encode(demul_alpha(srgb_gamma_decode(textureLoad(source_texture, src_coords))));
    result = select(vec4f(), color, rectangle_contains(source, position));
}
