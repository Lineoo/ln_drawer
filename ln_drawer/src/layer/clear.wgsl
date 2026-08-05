@group(0) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    textureStore(destination_texture, vec2i(id.xy), vec4f());
}
