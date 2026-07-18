struct Uniforms {
    projection: mat4x4<f32>,
    light: vec4<f32>,
    stock_size: vec4<f32>,
    window_size: vec2<f32>,
    view: u32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) xy: vec2<f32>,
    @location(1) z: u32,
    @location(2) face: u32,
};

struct InstanceInput {
    @location(3) center: vec2<f32>,
    @location(4) height: f32,
    @location(5) faces: u32,
}

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec3<f32>,
};

fn clipped() -> VertexOutput {
    var clipped: VertexOutput;
    clipped.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return clipped;
}

@vertex
fn vs_main(vertex: VertexInput, voxel: InstanceInput) -> VertexOutput {
    if voxel.height <= 0.0 {
        return clipped();
    }

    if (voxel.faces & vertex.face) == 0u {
        return clipped();
    }

    let window_size = uniforms.window_size;
    let xy = voxel.center + vertex.xy;
    let z = voxel.height * f32(vertex.z);

    let world = uniforms.projection * vec4<f32>(xy, z, 1.0);

    var out: VertexOutput;
    var color: vec3<f32>;

    if (vertex.face & 1 << 1) != 0 {
        color = vec3<f32>(0.5, 0.5, 0.5);
    } else if (vertex.face & 1 << 2) != 0 {
        color = vec3<f32>(0.9, 0.9, 0.9);
    } else {
        color = vec3<f32>(0.1, 0.1, 0.1);
    }

    // convert to ndc
    // direction already match ndc
    out.clip_position = vec4<f32>((world.xy / window_size * 2.0), world.z, 1.0);
    out.color = color;

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
