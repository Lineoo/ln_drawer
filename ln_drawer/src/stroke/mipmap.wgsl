// include! dispatch colorspace

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;
@group(2) @binding(0) var source: texture_storage_2d<rgba8unorm, read_write>;
@group(2) @binding(1) var<uniform> source_key: vec3i;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    let src_texl_size = i32(exp2(f32(source_key.z)));
    let src_real_size = texture_base_size * src_texl_size;
    let src_chunk_min = (source_key.xy) * src_real_size;
    let src_chunk_max = (source_key.xy + vec2i(1)) * src_real_size;
    let area = area(id);
    if area.x < src_chunk_min.x || area.y < src_chunk_min.y || area.x >= src_chunk_max.x || area.y >= src_chunk_max.y { return; }

    let smol = coords(id) % 256 * 2;

    let c0 = srgb_to_linear(textureLoad(source, smol));
    let c1 = srgb_to_linear(textureLoad(source, smol + vec2i(0, 1)));
    let c2 = srgb_to_linear(textureLoad(source, smol + vec2i(1, 1)));
    let c3 = srgb_to_linear(textureLoad(source, smol + vec2i(1, 0)));

    let a = c0.a + c1.a + c2.a + c3.a;
    if a < 1e-6 { return; }

    let rgb = (c0.rgb * c0.a + c1.rgb * c1.a + c2.rgb * c2.a + c3.rgb * c3.a) / a;
    textureStore(destination, coords(id), linear_to_srgb(vec4f(rgb, a / 4)));
}
