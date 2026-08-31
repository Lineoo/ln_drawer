#lib_rectangle

@group(0) @binding(0) var<uniform> sample: vec2i;
@group(0) @binding(1) var<storage> result: vec4f;

@group(1) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(1)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let position = sample + vec2i(id.xy);
    let src_coords = position - source.coords;

    result = select(vec4f(), textureLoad(source_texture, src_coords), rectangle_contains(source, position));
}
