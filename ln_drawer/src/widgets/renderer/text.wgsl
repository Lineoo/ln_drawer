@group(1) @binding(1) var texture: texture_2d<f32>;
@group(1) @binding(2) var texture_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let size = vec2f(textureDimensions(texture));
    let point = size * uv;
    
    var value = vec4f();
    value += textureSample(texture, texture_sampler, (point + vec2f(1.0, 1.0)) / size) * 0.7;
    value += textureSample(texture, texture_sampler, (point + vec2f(-1.0, 1.0)) / size) * 0.7;
    value += textureSample(texture, texture_sampler, (point + vec2f(-1.0, 1.0)) / size) * 0.7;
    value += textureSample(texture, texture_sampler, (point + vec2f(-1.0, -1.0)) / size) * 0.7;
    value += textureSample(texture, texture_sampler, (point + vec2f(1.0, 0.0)) / size) * 1.0;
    value += textureSample(texture, texture_sampler, (point + vec2f(0.0, 1.0)) / size) * 1.0;
    value += textureSample(texture, texture_sampler, (point + vec2f(-1.0, 0.0)) / size) * 1.0;
    value += textureSample(texture, texture_sampler, (point + vec2f(0.0, -1.0)) / size) * 1.0;
    value += textureSample(texture, texture_sampler, uv) * 16.0;
    value /= 22.8;

    return smoothstep(vec4f(0.4), vec4f(0.6), value);
}