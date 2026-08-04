// Line
//
// Draws a straight **anti-aliased** line instance of specified color.
//
// Reference:
// https://github.com/KaNaDaAT/vega-webgpu/blob/main/src/shaders/line.wgsl

struct Uniforms {
    projection: mat4x4<f32>,
    stock_size: vec4<f32>,
    window_size: vec2<f32>,
    view: u32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Vertex {
    @location(0) vertex: u32,
};

struct Instance {
    @location(1) start: vec3<f32>,
    @location(2) end: vec3<f32>,
    @location(3) move_type: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec3<f32>, // each pixel gets the same color
    @location(1) center: f32, // distance from center
};

const RAPID_MOVE_COLOR = vec3<f32>(1.0, 0.1, 0.1);
const FEED_MOVE_COLOR = vec3<f32>(0.1, 1.0, 0.1);

const STROKE_WIDTH = 2.0;
const SMOOTHING = 1.0; // width of are on each side of line that is used to fade the line, ie, the area with alpha changes

@vertex
fn vs_main(quad: Vertex, instance: Instance) -> VertexOutput {
    let window_size = uniforms.window_size;

    // scaled to fit the screen, in pixels
    let start = uniforms.projection * vec4<f32>(instance.start, 1.0);
    let end = uniforms.projection * vec4<f32>(instance.end, 1.0);

    // unit vector from start to end
    let dir = normalize(end - start);
    // normal vector, to get perpendicular direction
    let normal = vec2<f32>(-dir.y, dir.x);

    // vertex: 0,2 = -normal
    // vertex: 1,3 = +normal
    // halfs the normal vector and adds sign to it
    let side = select(0.5, -0.5, quad.vertex % 2 == 0);

    let offset = normal * STROKE_WIDTH * side;
    // use first two vertex invocations for start side
    let pos = select(start.xy, end.xy, quad.vertex > 1) + offset;
    // make sure that the toolpath is a little above the stock
    let depth = select(start.z, end.z, quad.vertex > 1) - 0.001;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos / uniforms.window_size * 2.0, depth, 1.0);
    out.color = select(FEED_MOVE_COLOR, RAPID_MOVE_COLOR, instance.move_type == 0);
    out.center = side;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let offset = abs(in.center) * STROKE_WIDTH; // distance from center in pixels
    let stroke = STROKE_WIDTH * 0.5; // half the stroke width in pixels

    let alpha = 1.0 - smoothstep(stroke - SMOOTHING, stroke, offset);

    return vec4<f32>(in.color, alpha);
}
