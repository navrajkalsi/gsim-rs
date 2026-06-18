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
    @location(0) pos: vec3<f32>,
};

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn clipped() -> VertexOutput {
    var clipped: VertexOutput;
    clipped.clip_position = vec4<f32>(1.1, 1.1, 1.1, 1.0);
    return clipped;
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32, in: VertexInput) -> VertexOutput {
    if index >= 36 {
        // not possible
        // clip out
        return clipped();
    }

    // one of the 6 faces of the cube
    let face = index / 6;
    // one of the 6 vertices to create 2 triangles which make a face
    let vertex = index % 6;

    let half = 100.0;

    // offsets for different corners
    let left_bottom_near = in.pos + vec3<f32>(-half, -half, -half);
    let left_bottom_far = in.pos + vec3<f32>(-half, -half, half);
    let left_top_near = in.pos + vec3<f32>(-half, half, -half);
    let left_top_far = in.pos + vec3<f32>(-half, half, half);
    let right_bottom_near = in.pos + vec3<f32>(half, -half, -half);
    let right_bottom_far = in.pos + vec3<f32>(half, -half, half);
    let right_top_near = in.pos + vec3<f32>(half, half, -half);
    let right_top_far = in.pos + vec3<f32>(half, half, half);

    let vertices = array(
        left_bottom_far, // bottom face
        left_bottom_near,
        right_bottom_far,
        right_bottom_far,
        left_bottom_near,
        right_bottom_near,
        left_top_far, // top face
        left_top_near,
        right_top_far,
        right_top_far,
        left_top_near,
        right_top_near,
        left_bottom_near,// left face
        left_bottom_far,
        left_top_near,
        left_top_near,
        left_bottom_far,
        left_top_far,
        right_bottom_near,// right face
        right_bottom_far,
        right_top_near,
        right_top_near,
        right_bottom_far,
        right_top_far,
        left_top_far,// top face
        left_top_near,
        right_top_far,
        right_top_far,
        left_top_near,
        left_bottom_far,// bottom face
        left_bottom_near,
        right_bottom_far,
        right_bottom_far,
        left_bottom_near,
        right_bottom_near,
        right_bottom_near,
    );

    let window_size = uniforms.window_size;

    // convert position to number of pixels
    let position = uniforms.projection * vec4<f32>(vertices[index], 1.0);

    var out: VertexOutput;
    out.clip_position = vec4<f32>((position.xy / window_size * 2.0), 0.0, 1.0);
    out.color = vec4<f32>(0.5, 0.5, 0.5, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
