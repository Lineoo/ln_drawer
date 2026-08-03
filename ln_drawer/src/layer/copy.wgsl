struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: two textures are the same pixel size
    let position = dispatch.coords + vec2i(id.xy);

    if (any(position < dispatch.coords)) { return; }
    if (any(position - dispatch.coords >= vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position - destination.coords >= vec2i(destination.size))) { return; }

    if (any(position < source.coords)) { return; }
    if (any(position - source.coords >= vec2i(source.size))) { return; }
    
    let src_coords = position - source.coords;
    let dst_coords = position - destination.coords;

    textureStore(destination_texture, dst_coords, textureLoad(source_texture, src_coords));
}
