// Stock
//
// Draws a cuboidal stock instance of specified height.

struct Uniforms {
    matrix: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Immediates {
    stock_height: f32,
};
var<immediate> immediates: Immediates;

struct Vertex {
    @location(0) xy: vec2<f32>,
    @location(1) z: u32,
    @location(2) face: u32,
};

struct Instance {
    @location(3) center: vec2<f32>,
    @location(4) height: f32,
    @location(5) faces: u32,
}

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
};

const TOP = 1 << 0;

fn clipped() -> VertexOutput {
    var clipped: VertexOutput;
    clipped.clip_pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return clipped;
}

@vertex
fn vs_main(vertex: Vertex, instance: Instance) -> VertexOutput {
    if instance.height <= 0.0 {
        return clipped();
    }

    if (instance.faces & vertex.face) == 0u {
        return clipped();
    }

    let xy = instance.center + vertex.xy;
    let z = select(instance.height, 0.0, vertex.z == 0);

    var out: VertexOutput;
    out.clip_pos = uniforms.matrix * vec4<f32>(xy, z, 1.0);

    if vertex.face == TOP {
        // top face gets dimmer with depth
        // useful for perceiving depth from top view
        let rel_height = (instance.height / immediates.stock_height) / 2.0;
        let color_seg = 0.3 + rel_height;
        out.color = vec4<f32>(color_seg, color_seg, color_seg, 1.0);
    } else {
        // color all side faces darker
        out.color = vec4<f32>(0.25, 0.25, 0.25, 1.0);
    }

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
