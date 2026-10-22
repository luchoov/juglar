// Bloom: the glow around bright things, as a camera lens or the eye makes
// it. See `bloom.rs` for the chain this step builds.
//
// `common.wgsl` comes before this file.

@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

// Half the size: each output pixel averages a 4×4 block of the source, read
// as four filtered samples, each blending four pixels.
@fragment
fn fs_down(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let texel = 1.0 / vec2f(textureDimensions(source));
    // This pixel's center lands on the corner between four source pixels.
    let uv = position.xy * 2.0 * texel;
    var c = textureSampleLevel(source, smooth_sampler, uv + vec2f(-texel.x, -texel.y), 0.0).rgb;
    c += textureSampleLevel(source, smooth_sampler, uv + vec2f(texel.x, -texel.y), 0.0).rgb;
    c += textureSampleLevel(source, smooth_sampler, uv + vec2f(-texel.x, texel.y), 0.0).rgb;
    c += textureSampleLevel(source, smooth_sampler, uv + vec2f(texel.x, texel.y), 0.0).rgb;
    return vec4f(0.25 * c, 1.0);
}
