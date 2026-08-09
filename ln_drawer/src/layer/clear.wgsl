// include! rectangle

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let start = max(dispatch.coords, destination.coords);
    let position = start + vec2i(id.xy);

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position);
    if !validated { return; }

    let dst_coords = position - destination.coords;
    textureStore(destination_texture, dst_coords, vec4f());
}
