struct Uniforms {
    projection: mat4x4<f32>,
    stock_size: vec4<f32>,
    window_size: vec2<f32>,
    view: u32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) vertex: vec3<f32>,
};

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) height: f32,
}

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec3<f32>,
};

fn clipped() -> VertexOutput {
    var clipped: VertexOutput;
    clipped.clip_position = vec4<f32>(1.1, 1.1, 1.1, 1.0);
    return clipped;
}

@vertex
fn vs_main(cube: VertexInput, instance: InstanceInput) -> VertexOutput {
    if instance.height <= 0.0 {
        return clipped();
    }

    let window_size = uniforms.window_size;
    let center = vec2<f32>(instance.center + cube.vertex.xy);
    let height = instance.height * cube.vertex.z;

    let world = uniforms.projection * vec4<f32>(center, height, 1.0);

    var out: VertexOutput;

    // convert to ndc
    // direction already match ndc
    out.clip_position = vec4<f32>((world.xy / window_size * 2.0), 0.7, 1.0);
    out.color = vec3<f32>(1.0, 1.0, 1.0);

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
