// source: https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl

struct Uniforms {
    window_size: vec2<f32>,
    max_travels: vec3<f32>,
    stroke_width: f32,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// mark as a valid vertex shader
@vertex
fn vs_main(@builtin(vertex_index) index: u32, in: VertexInput) -> VertexOutput {
    let start = in.start;
    let end = in.end;
    let stroke_width = uniforms.stroke_width;

    let positions = array(
        vec2<f32>(start.x, start.y - stroke_width),
        vec2<f32>(start.x, start.y + stroke_width),
        vec2<f32>(end.x, end.y - stroke_width),
        vec2<f32>(end.x, end.y - stroke_width),
        vec2<f32>(end.x, end.y + stroke_width),
        vec2<f32>(start.x, start.y + stroke_width),
    );

    var out: VertexOutput;

    out.color = in.color;
    out.clip_position = vec4<f32>(positions[index], 0.0, 1.0);

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
