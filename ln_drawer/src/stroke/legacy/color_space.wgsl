struct DrawMeta {
    draw_coords: vec2i,
    draw_size: vec2u,
}

@group(0) @binding(0) var<uniform> drawing: DrawMeta;

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> destination_key: vec3i;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let texl_size = i32(exp2(f32(destination_key.z)));
    let real_size = texture_size * texl_size;
    let chunk_min = (destination_key.xy) * real_size;
    let chunk_max = (destination_key.xy + vec2i(1)) * real_size;

    let area_min = drawing.draw_coords;
    let area_max = drawing.draw_coords + vec2i(drawing.draw_size);

    let coords_min = (real_size + area_min - chunk_min) / texl_size - texture_size;
    let coords_max = (real_size + area_max - chunk_min - 1) / texl_size + 1 - texture_size;

    let area = area_min + vec2i(id.xy) * texl_size;
    let coords = coords_min + vec2i(id.xy);

    if coords.x >= coords_max.x || coords.y >= coords_max.y { return; }
    if area.x < chunk_min.x || area.y < chunk_min.y || area.x >= chunk_max.x || area.y >= chunk_max.y { return; }

    let color = textureLoad(destination, coords);
    textureStore(destination, coords, linear_to_srgb(color));
}

fn linear_to_srgb(v: vec4f) -> vec4f {
    let threshold = vec3(0.0031308);
    let low = v.rgb * 12.92;
    let high = 1.055 * pow(v.rgb, vec3(1.0 / 2.4)) - 0.055;
    return vec4f(select(high, low, v.rgb < threshold), v.a);
}