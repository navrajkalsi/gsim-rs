// Line
//
// Draws a straight line of specified thickness and color.
// Depth of each line can be specified in range 0..1, 0.0 being the nearest plane.
//
// Reference:
// https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl

struct Uniforms {
    window_size: vec2<f32>,
    _pad: vec2<f32>,
    max_travels: vec4<f32>,
    projection: mat4x4<f32>,
    tool_color: vec4<f32>,
    tool_size: f32,
    tool_len: f32,
    view: u32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) vertex: vec2<i32>,
};

struct InstanceInput {
    @location(1) start: vec3<f32>,
    @location(2) end: vec3<f32>,
    @location(3) color: vec3<f32>,
    @location(4) stroke_width: f32,
    @location(5) depth: f32,
}

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

const smooth_step = 1.5;

// mark as a valid vertex shader
@vertex
fn vs_main(quad: VertexInput, instance: InstanceInput) -> VertexOutput {
    // exactly 0 stroke width is intentional and meant when the vertex is not to be shown
    if instance.stroke_width == 0.0 {
        var clipped: VertexOutput;
        clipped.clip_position = vec4<f32>(1.1, 1.1, 1.1, 1.0);
        return clipped;
    }

    let window_size = uniforms.window_size;
    // unit vector from start to end
    let dir = normalize(instance.end.xy - instance.start.xy);
    // normal vector, to get perpendicular direction, with magnitude of stroke width
    let normal = vec2<f32>(-dir.y, dir.x);
    let tangent = dir;

    let offset = f32(quad.vertex.x) * tangent +
    f32(quad.vertex.y) * normal * instance.stroke_width * 0.5;

    let world = mix(instance.start.xy, instance.end.xy, 0.5) + offset;

    var out: VertexOutput;
    out.color = instance.color;

    out.clip_position = uniforms.projection * vec4<f32>(world, instance.depth, 1.0);

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
