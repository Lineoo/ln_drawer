// include! colorspace dispatch

@group(1) @binding(0) var source: texture_storage_2d<rgba8unorm, read_write>;
@group(2) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    let src = srgb_to_linear(textureLoad(source, coords(id)));
    let dst = srgb_to_linear(textureLoad(destination, coords(id)));

    let a = src.a + dst.a * (1.0 - src.a);
    if a < 1e-6 {
        textureStore(destination, coords(id), vec4f(0));
    } else {
        let rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / a;
        textureStore(destination, coords(id), linear_to_srgb(vec4f(rgb, a)));
    }
}

@compute @workgroup_size(16, 16)
fn cs_erase(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    let src = srgb_to_linear(textureLoad(source, coords(id)));
    let dst = srgb_to_linear(textureLoad(destination, coords(id)));

    let a = dst.a * (1.0 - src.a);
    if a < 1e-6 {
        textureStore(destination, coords(id), vec4f(0));
    } else {
        textureStore(destination, coords(id), linear_to_srgb(vec4f(dst.rgb, a)));
    }
}
