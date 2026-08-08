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
    center: mat4x4<f32>,
    scale: mat4x4<f32>,
    x_rotation: mat4x4<f32>,
    y_rotation: mat4x4<f32>,
    z_rotation: mat4x4<f32>,
    stock_size: vec4<f32>,
    window_size: vec2<f32>,
    bounding_cube_edge: f32,
    user_scale: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Instance {
    @location(0) pos: vec3<f32>,
    @location(1) diameter: f32,
    @location(2) length: f32,
};

fn clipped() -> vec4<f32> {
    return vec4<f32>(2.0, 2.0, 2.0, 1.0);
}

// angle must be multiple of 4 as we need to draw 4 triangles for each unit degree to create a
// cylinder
@vertex
fn vs_main(@builtin(vertex_index) index: u32, instance: Instance) -> @builtin(position) vec4<f32> {
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

    let tool_rad = instance.diameter / 2.0;
    let tool_len = instance.length;

    var position = vec4<f32>(instance.pos, 1.0);

    switch vertex + face * 3u {
        case 11 {
            // bottom circle center
            position.x += 0.0;
            position.y += 0.0;
            position.z += 0.0;
        }
        case 10 {
            // bottom circle perimeter point at curent angle
            position.x += tool_rad * cos(angle);
            position.y += tool_rad * sin(angle);
            position.z += 0.0;
        }
        case 9 {
            // bottom circle perimeter point at next unit degree angle
            position.x += tool_rad * cos(angle_next);
            position.y += tool_rad * sin(angle_next);
            position.z += 0.0;
        }
        case 8 {
            // first wall triangle bottom
            position.x += tool_rad * cos(angle);
            position.y += tool_rad * sin(angle);
            position.z += 0.0;
        }
        case 7 {
            // first wall triangle top first
            position.x += tool_rad * cos(angle);
            position.y += tool_rad * sin(angle);
            position.z += tool_len;
        }
        case 6 {
            // first wall triangle top second
            position.x += tool_rad * cos(angle_next);
            position.y += tool_rad * sin(angle_next);
            position.z += tool_len;
        }
        case 5 {
            // second wall triangle top
            position.x += tool_rad * cos(angle_next);
            position.y += tool_rad * sin(angle_next);
            position.z += tool_len;
        }
        case 4 {
            // second wall triangle bottom first
            position.x += tool_rad * cos(angle_next);
            position.y += tool_rad * sin(angle_next);
            position.z += 0.0;
        }
        case 3 {
            // second wall triangle bottom second
            position.x += tool_rad * cos(angle);
            position.y += tool_rad * sin(angle);
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
            position.x += tool_rad * cos(angle);
            position.y += tool_rad * sin(angle);
            position.z += tool_len;
        }
        case 0u {
            // top circle perimeter point at next unit degree angle
            position.x += tool_rad * cos(angle_next);
            position.y += tool_rad * sin(angle_next);
            position.z += tool_len;
        }
        default {
            return clipped();
        }
    }

    let window_size = uniforms.window_size;

    // convert position to number of pixels
    position = uniforms.scale * (uniforms.x_rotation * (uniforms.y_rotation * (uniforms.z_rotation * (uniforms.center *
    position))));
    position.x /= window_size.x;
    position.y /= window_size.y;

    return position;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.1, 0.1, 0.1, 1.0);
}
