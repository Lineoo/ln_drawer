// include! dispatch

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;
@group(2) @binding(0) var source: texture_storage_2d<rgba8unorm, read_write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    let smol = coords(id) % 256 * 2;

    let c0 = textureLoad(source, smol);
    let c1 = textureLoad(source, smol + vec2i(0, 1));
    let c2 = textureLoad(source, smol + vec2i(1, 1));
    let c3 = textureLoad(source, smol + vec2i(1, 0));

    let a = c0.a + c1.a + c2.a + c3.a;
    if a < 1e-6 { return; }

    let rgb = (c0.rgb * c0.a + c1.rgb * c1.a + c2.rgb * c2.a + c3.rgb * c3.a) / a;
    textureStore(destination, coords(id), vec4f(rgb, a / 4));
}
