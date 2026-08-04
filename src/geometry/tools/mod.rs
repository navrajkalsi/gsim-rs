use crate::{config::ToolConfig, geometry::line::LineInstance, points::Point};

/// Represents the current 3D position of the tool,
/// that can be drawn to the screen with a vertex shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToolInstance {
    position: [f32; 3],
    diameter: f32,
    length: f32,
}

impl ToolInstance {
    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`ToolInstance`] is stored in a GPU buffer.
    ///
    /// The layout is set to use [`VertexStepMode::Instance`](wgpu::VertexStepMode::Instance),
    /// which allows the vertex shader to expand a single 3D point to a cylinderical tool,
    /// with its bottom center at the tool position.
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }

    /// Creates a new [`ToolInstance`], which will be rendered at the provided [`Point`].
    pub fn at_point(point: Point, tool_config: ToolConfig) -> Self {
        Self {
            position: [point.x, point.y, point.z],
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
