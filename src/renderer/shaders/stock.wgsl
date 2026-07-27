// Stock
//
// Draws a cuboidal stock instance of specified height.

struct Uniforms {
    projection: mat4x4<f32>,
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

const TOP = 1 << 0;

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
    let stock_height = uniforms.stock_size.z;

    let xy = voxel.center + vertex.xy;
    let z = voxel.height * f32(vertex.z);

    let world = uniforms.projection * vec4<f32>(xy, z, 1.0);

    var out: VertexOutput;

    // convert to ndc
    // direction already match ndc
    out.clip_position = vec4<f32>((world.xy / window_size * 2.0), world.z, 1.0);

    if vertex.face == TOP {
        // top face gets dimmer with depth
        // useful for perceiving depth from top view
        let rel_height = (voxel.height / stock_height) / 1.5;
        let color_seg = 0.3 + rel_height;
        out.color = vec3<f32>(color_seg, color_seg, color_seg);
    } else {
        // color all side faces darker
        out.color = vec3<f32>(0.25, 0.25, 0.25);
    }

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
