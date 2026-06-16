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
    if index >= 12 {
        // not possible
        // clip out
        return clipped();
    }

    var position = vec4<f32>(in.pos, 1.0);
    let half = 12.5;

    switch index {
        case 0 {
            position.x -= half;
            position.y += half;
            position.z += half;
        }
        case 1 {
            position.x -= half;
            position.y -= half;
            position.z += half;
        }
        case 2 {
            position.x -= half;
            position.y += half;
            position.z -= half;
        }
        case 3 {
            position.x -= half;
            position.y += half;
            position.z += half;
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
    out.clip_position = vec4<f32>((position.xy / window_size * 2.0), 0.0, 1.0);
    out.color = uniforms.tool_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
