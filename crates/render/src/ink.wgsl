// Text and panels over the finished frame (see `ink.rs`). Each instance is
// one quad: a glyph sampled from the atlas, or a panel with rounded corners
// cut by its distance to the edge. The frame underneath is linear light.

struct Ink {
    resolution: vec2<f32>,
}

@group(0) @binding(0) var<uniform> ink: Ink;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Quad {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shape: vec4<f32>,
}

struct Corner {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Pixels from the quad's center, and its half size.
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) half_size: vec2<f32>,
    @location(3) @interpolate(flat) color: vec4<f32>,
    @location(4) @interpolate(flat) shape: vec4<f32>,
}

@vertex
fn vs_quad(@builtin(vertex_index) index: u32, quad: Quad) -> Corner {
    // A strip of four corners: top left, bottom left, top right, bottom right.
    let corner = vec2<f32>(f32(index / 2u), f32(index % 2u));
    let pixel = quad.rect.xy + corner * quad.rect.zw;
    var out: Corner;
    out.position = vec4<f32>(pixel / ink.resolution * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = mix(quad.uv.xy, quad.uv.zw, corner);
    out.half_size = quad.rect.zw * 0.5;
    out.local = (corner - 0.5) * quad.rect.zw;
    out.color = quad.color;
    out.shape = quad.shape;
    return out;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_quad(in: Corner) -> @location(0) vec4<f32> {
    var coverage: f32;
    if in.shape.y > 0.5 {
        // Blended in linear light, antialiased edges come out thin; a
        // little gamma on the coverage gives them back their weight.
        coverage = pow(textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r, 1.0 / 1.45);
    } else {
        let radius = min(in.shape.x, min(in.half_size.x, in.half_size.y));
        let q = abs(in.local) - in.half_size + radius;
        let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
        coverage = clamp(0.5 - distance, 0.0, 1.0);
    }
    let alpha = in.color.a * coverage;
    return vec4<f32>(to_linear(in.color.rgb) * alpha, alpha);
}
