struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

fn rectangle_contains(r: Rectangle, p: vec2i) -> bool {
    return all(p >= r.coords) & all(p - r.coords < vec2i(r.size));
}