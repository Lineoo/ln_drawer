@group(0) @binding(3) var<storage, read_write> draws_state: array<vec4f>;

@compute @workgroup_size(1)
fn cs_main() {
    draws_state[0] = vec4f();
}
