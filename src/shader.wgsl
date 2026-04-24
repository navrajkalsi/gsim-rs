// reference for stroke width:
// https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl

struct Uniforms {
    window_size: vec2<f32>,
    padding: vec2<f32>,
    max_travels: vec4<f32>,
    scale: f32,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// add machine boundary by default some way

struct VertexInput {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) stroke_width: f32};

struct VertexOutput {
    // builtin position means that the value is to be used for clip_position
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// mark as a valid vertex shader
@vertex
fn vs_main(@builtin(vertex_index) index: u32, in: VertexInput) -> VertexOutput {
    let stroke_width = in.stroke_width;
    let scale = uniforms.scale;
    let padding = uniforms.padding;

    // number of pixels from the machine zero corner of screen
    // (this corner may or may not be the 0 points of the window)
    var start = in.start * scale;
    var end = in.end * scale;

    // machine size in pixels
    let machine_size = abs(uniforms.max_travels * scale);

    // position on screen with respect to the window coordinate system(0 on top left corner)
    // without padding
    if uniforms.max_travels.x >= 0.0 {
        if uniforms.max_travels.y >= 0.0 {
            // machine zero on lower left corner, all positive vals
            start.x += padding.x;
            start.y = (uniforms.window_size.y - start.y) - padding.y;
            end.x += padding.x;
            end.y = (uniforms.window_size.y - end.y) - padding.y;
        } else {
            // machine zero on top left corner, negative y vals
            start.x += padding.x;
            start.y = abs(start.y) + padding.y;
            end.x += padding.x;
            end.y = abs(end.y) + padding.y;
        }
    } else {
        if uniforms.max_travels.y >= 0.0 {
            // machine zero on lower right corner, negative x vals
            start.x = (uniforms.window_size.x - abs(start.x)) + padding.x;
            start.y += padding.y;
            end.x = (uniforms.window_size.x - abs(end.x)) + padding.x;
            end.y += padding.y;
        } else {
            // machine zero on top right corner, all negative vals
            start.x = (uniforms.window_size.x - abs(start.x)) + padding.x;
            start.y = abs(start.y) + padding.y;
            end.x = (uniforms.window_size.x - abs(end.x)) + padding.x;
            end.y = abs(end.y) + padding.y;
        }
    };

    // flip y to match coordinate system of clip space
    start.x = (start.x / uniforms.window_size.x) * 2.0 - 1.0;
    start.y = 1.0 - (start.y / uniforms.window_size.y) * 2.0;
    end.x = (end.x / uniforms.window_size.x) * 2.0 - 1.0;
    end.y = 1.0 - (end.y / uniforms.window_size.y) * 2.0;

    // unit vector from start to end
    let dir = normalize(end - start);
    // normal vector, to get perpendicular direction, with magnitude of stroke width
    let normal = vec2<f32>(-dir.y, dir.x) * stroke_width / 2.0;

    var p1 = start.xy - normal;
    var p2 = start.xy + normal;
    var p3 = end.xy - normal;
    var p4 = end.xy + normal;

    let positions = array(
        p1, p2, p3, p2, p4, p3
    );

    var out: VertexOutput;

    out.color = in.color;
    out.clip_position = vec4<f32>(positions[index].xy, 0.0, 1.0);

    return out;
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
