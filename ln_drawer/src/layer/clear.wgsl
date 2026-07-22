@group(0) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    textureStore(destination, vec2i(id.xy), vec4f(0));
}
