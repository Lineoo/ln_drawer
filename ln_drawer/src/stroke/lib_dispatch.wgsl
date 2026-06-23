struct DispatchMeta {
    dispatch_coords: vec2i,
    dispatch_size: vec2u,
}

const texture_base_size: i32 = 512;

@group(0) @binding(0) var<uniform> dispatch: DispatchMeta;
@group(1) @binding(1) var<uniform> destination_key: vec3i;

fn pixel_size() -> i32 {
    return i32(exp2(f32(destination_key.z)));
}

fn texel_size() -> i32 {
    return texture_base_size * pixel_size();
}

fn chunk_coords() -> vec2i {
    return (destination_key.xy) * texel_size();
}

// _area_ is world-space coords of dispatch

fn area_min() -> vec2i {
    return dispatch.dispatch_coords;
}

fn area_max() -> vec2i {
    return dispatch.dispatch_coords + vec2i(dispatch.dispatch_size);
}

fn area(id: vec3u) -> vec2i {
    return area_min() + vec2i(id.xy) * pixel_size();
}

fn area_satisfied(id: vec3u) -> bool {
    return all(vec4(area(id) >= area_min(), area(id) < area_max()));
}

// _coords_ is texture-space coords of dispatch

fn coords_min() -> vec2i {
    return (texel_size() + area_min() - chunk_coords()) / pixel_size() - texture_base_size;
}

fn coords_max() -> vec2i {
    return (texel_size() + area_max() - chunk_coords() - 1) / pixel_size() + 1 - texture_base_size;
}

fn coords(id: vec3u) -> vec2i {
    return coords_min() + vec2i(id.xy);
}

fn coords_satisfied(id: vec3u) -> bool {
    return all(vec4(coords(id) >= coords_min(), coords(id) < coords_max()));
}
