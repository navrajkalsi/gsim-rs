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
    @location(0) vertex: vec3<f32>,
};

struct InstanceInput {
    @location(1) center: vec3<f32>,
}

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(cube: VertexInput, instance: InstanceInput) -> VertexOutput {
    let window_size = uniforms.window_size;

    let world = uniforms.projection * vec4<f32>((instance.center + cube.vertex), 1.0);

    var out: VertexOutput;

    // convert to ndc
    // direction already match ndc
    out.clip_position = vec4<f32>((world.xy / window_size * 2.0), 0.0, 1.0);
    out.color = vec3<f32>(0.0, 0.0, 0.0);

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
