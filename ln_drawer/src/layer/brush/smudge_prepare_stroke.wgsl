#lib_constant #lib_rectangle #lib_colorspace

struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
    color_ratio: f32,
    sample_radius: f32,
    sample_rate: f32,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;
@group(0) @binding(3) var<storage, read_write> draws_state: array<vec4f>;

// Seed the carried smudge color with the foreground of the first dab so that a
// stroke always starts from the brush color before picking up the canvas.
@compute @workgroup_size(1)
fn cs_main() {
    if draws_length == 0u { return; }
    draws_state[0] = vec4f();
}
