// source: https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl

struct Uniforms {
    window_size: vec2<f32>,
    padding: vec2<f32>,
    max_travels: vec4<f32>,
    scale: f32,
    stroke_width: f32,
    _pad: vec2<f32>};
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
    let stroke_width = uniforms.stroke_width;
    let scale = uniforms.scale;
    let padding = uniforms.padding;

    // number of pixels from the machine zero corner of screen
    // (this corner may or may not be the 0 points of the window)
    var start = abs(in.start * scale);
    var end = abs(in.end * scale);

    // machine size in pixels
    let machine_size = abs(uniforms.max_travels * scale);

    // position on screen with respect to the window coordinate system(0 on top left corner)
    // without padding
    if uniforms.max_travels.x >= 0.0 {
        if uniforms.max_travels.y >= 0.0 {
            // machine zero on lower left corner, all positive vals
            start.x += padding.x;
            start.y = (uniforms.window_size.y - start.y) + padding.y;
            end.x += padding.x;
            end.y = (uniforms.window_size.y - end.y) + padding.y;
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

    start.x = (start.x / uniforms.window_size.x) * 2.0 - 1.0;
    start.y = 1.0 - (start.y / uniforms.window_size.y) * 2.0;
    end.x = (end.x / uniforms.window_size.x) * 2.0 - 1.0;
    end.y = 1.0 - (end.y / uniforms.window_size.y) * 2.0;

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
