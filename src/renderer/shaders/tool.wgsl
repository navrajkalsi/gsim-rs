// Tool
//
// Draws a cylinder whose bottom circular face is centered at `VertexInput.pos`.
// This is done by drawing 4 triangles per 10 degrees sweep at the axis of the cylinder.
//
// Top and bottom faces get one triangle each and the curved face is drawn with a rectangle,
// built using two triangles.
// Therefore each 10 degrees of the cylinder is drawing using 12 vertices.
//
// The tool is drawn at depth `0.0`, which is the nearest plane.

struct Uniforms {
    matrix: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Instance {
    @location(1) position: vec3<f32>,
    @location(2) diameter: f32,
    @location(3) length: f32,
};

@vertex
fn vs_main(@location(0) vertex: vec3<f32>, instance: Instance) -> @builtin(position) vec4<f32> {
    let rad = instance.diameter / 2.0;
    let pos = instance.position;
    let len = instance.length;

    return uniforms.matrix * vec4<f32>(pos.x + rad * vertex.x, pos.y + rad * vertex.y, pos.z +
    len * vertex.z, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.1, 0.1, 0.1, 1.0);
}
