// include! colorspace, dispatch

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    let color = textureLoad(destination, coords(id));
    textureStore(destination, coords(id), linear_to_srgb(color));
}