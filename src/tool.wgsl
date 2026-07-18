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
    projection: mat4x4<f32>,
    light: vec4<f32>,
    stock_size: vec4<f32>,
    window_size: vec2<f32>,
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

// angle must be multiple of 4 as we need to draw 4 triangles for each unit degree to create a
// cylinder
@vertex
fn vs_main(@builtin(vertex_index) index: u32, in: VertexInput) -> VertexOutput {
    // total triangles = 360/10 * 4
    // total vertices = 360/10 * 4 * 3 = 432
    if index >= 432 {
        // not possible
        // clip out
        return clipped();
    }

    // triangle num to draw
    let triangle = index / 3;
    // vertex of triangle to draw
    let vertex = index % 3;
    // what face of the cylinder does this triangle draw at a particular degree
    let face = triangle / 36;

    // angle in radians
    // each triangle covers 10 degrees
    let angle = radians(f32(triangle % 36) * 10.0);
    let angle_next = radians(f32(triangle % 36) * 10.0 + 10.0);

    let tool_size = 10.0;
    let tool_len = 125.0;

    var position = vec4<f32>(in.pos, 1.0);

    switch vertex + face * 3u {
        case 11 {
            // bottom circle center
            position.x += 0.0;
            position.y += 0.0;
            position.z += 0.0;
        }
        case 10 {
            // bottom circle perimeter point at curent angle
            position.x += tool_size * cos(angle);
            position.y += tool_size * sin(angle);
            position.z += 0.0;
        }
        case 9 {
            // bottom circle perimeter point at next unit degree angle
            position.x += tool_size * cos(angle_next);
            position.y += tool_size * sin(angle_next);
            position.z += 0.0;
        }
        case 8 {
            // first wall triangle bottom
            position.x += tool_size * cos(angle);
            position.y += tool_size * sin(angle);
            position.z += 0.0;
        }
        case 7 {
            // first wall triangle top first
            position.x += tool_size * cos(angle);
            position.y += tool_size * sin(angle);
            position.z += tool_len;
        }
        case 6 {
            // first wall triangle top second
            position.x += tool_size * cos(angle_next);
            position.y += tool_size * sin(angle_next);
            position.z += tool_len;
        }
        case 5 {
            // second wall triangle top
            position.x += tool_size * cos(angle_next);
            position.y += tool_size * sin(angle_next);
            position.z += tool_len;
        }
        case 4 {
            // second wall triangle bottom first
            position.x += tool_size * cos(angle_next);
            position.y += tool_size * sin(angle_next);
            position.z += 0.0;
        }
        case 3 {
            // second wall triangle bottom second
            position.x += tool_size * cos(angle);
            position.y += tool_size * sin(angle);
            position.z += 0.0;
        }
        case 2 {
            // top circle center
            position.x += 0.0;
            position.y += 0.0;
            position.z += tool_len;
        }
        case 1 {
            // top circle perimeter point at curent angle
            position.x += tool_size * cos(angle);
            position.y += tool_size * sin(angle);
            position.z += tool_len;
        }
        case 0u {
            // bottom circle perimeter point at next unit degree angle
            position.x += tool_size * cos(angle_next);
            position.y += tool_size * sin(angle_next);
            position.z += tool_len;
        }
        default {
            return clipped();
        }
    }

    let window_size = uniforms.window_size;

    // convert position to number of pixels
    position = uniforms.projection * position;

    var out: VertexOutput;
    out.clip_position = vec4<f32>((position.xy / window_size * 2.0), position.z, 1.0);
    out.color = vec4<f32>(0.25, 0.25, 0.25, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
