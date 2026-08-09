// include! rectangle

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: two textures are the same pixel size
    let start = max(max(dispatch.coords, destination.coords), source.coords);
    let position = start + vec2i(id.xy);

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position)
        && rectangle_contains(source, position);
    if !validated { return; }
    
    let src_coords = position - source.coords;
    let dst_coords = position - destination.coords;

    textureStore(destination_texture, dst_coords, textureLoad(source_texture, src_coords));
}
