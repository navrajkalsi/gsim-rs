//! # Tool
//!
//! The CPU representation of an instance of a **cylindrical** tool
//! that can be drawn to the screen with a vertex shader.

use crate::{config::ToolConfig, geometry::line::LineInstance, points::Point};
use std::f32::consts::PI;

/// Angle between consecutive vertices on the cylinder, in degrees.
/// Asserted to be a **factor of 360**.
const RESOLUTION: u8 = 10;

/// A single instance of a **cylindrical** tool,
/// with its base at [`Self::position`] and dynamic sizing.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToolInstance {
    /// Position of the center of the bottom face of tool.
    pub position: [f32; 3],

    /// Diameter of the tool.
    diameter: f32,

    /// Length of the tool.
    length: f32,
}

impl ToolInstance {
    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how
    /// a [`ToolInstance`] vertex is stored in a GPU buffer.
    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<[f32; 3]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }

    /// Collection of vertices required to construct all the individual faces of a [`ToolInstance`].
    ///
    /// The vertices are ordered in **counter-clockwise** order for each face.
    pub fn vertices() -> Vec<[f32; 3]> {
        debug_assert!(360 % RESOLUTION as usize == 0);

        // each section gets 4 triangles, therefore 12 vertices
        let sections = 360 / RESOLUTION as usize;
        let mut vertices = Vec::with_capacity(sections * 12);

        let res = RESOLUTION as f32 * PI / 180.0; // resolution in radians

        let mut next = res; // next angle

        // at start current angle is 0
        let mut current_cos = 1.0;
        let mut current_sin = 0.0;

        for _ in 0..sections {
            let next_cos = next.cos();
            let next_sin = next.sin();

            // bottom face
            vertices.push([0.0, 0.0, 0.0]); // center
            vertices.push([next_cos, next_sin, 0.0]); // next angle
            vertices.push([current_cos, current_sin, 0.0]); // current angle

            // first side face
            vertices.push([next_cos, next_sin, 1.0]); // next angle top
            vertices.push([current_cos, current_sin, 0.0]); // current angle
            vertices.push([next_cos, next_sin, 0.0]); // next angle

            // second side face
            vertices.push([current_cos, current_sin, 0.0]); // current angle
            vertices.push([next_cos, next_sin, 1.0]); // next angle top
            vertices.push([current_cos, current_sin, 1.0]); // current angle top

            // top face
            vertices.push([0.0, 0.0, 1.0]); // center
            vertices.push([current_cos, current_sin, 1.0]); // current angle
            vertices.push([next_cos, next_sin, 1.0]); // next angle

            next += res;
            current_cos = next_cos;
            current_sin = next_sin;
        }

        vertices
    }

    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`ToolInstance`] is stored in a GPU buffer.
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }

    /// Creates a new [`ToolInstance`], which will be rendered at the provided [`Point`].
    pub fn at_point(point: Point, tool_config: ToolConfig) -> Self {
        Self {
            position: point.as_array(),
            diameter: tool_config.diameter,
            length: tool_config.length,
        }
    }

    /// Creates a new [`ToolInstance`], which will be rendered at [`LineInstance::end`].
    pub fn at_line_end(instance: LineInstance, tool_config: ToolConfig) -> Self {
        Self {
            position: instance.end,
            diameter: tool_config.diameter,
            length: tool_config.length,
        }
    }
}
