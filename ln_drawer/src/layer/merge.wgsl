#lib_rectangle #lib_colorspace

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, #read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@group(3) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, #write>;
@group(3) @binding(1) var<uniform> swap: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: three textures are all the same pixel size
    let start = max(max(dispatch.coords, destination.coords), max(source.coords, swap.coords));
    let position = start + vec2i(id.xy);

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position)
        && rectangle_contains(source, position)
        && rectangle_contains(swap, position);
    if !validated { return; }

    let src_coords = position - source.coords;
    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    let dst = sensitive_blend_encode(textureLoad(destination_texture, dst_coords));
    let src = sensitive_blend_encode(textureLoad(source_texture, src_coords));

    let swp = #composite;

    textureStore(swap_texture, swp_coords, sensitive_blend_decode(swp));
}