use winit::dpi::PhysicalSize;

use crate::parser::Point;

const DEFAULT_STROKE_WIDTH: f32 = 0.01;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    start: [f32; 3],
    end: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        // 3d start and end points
        // with opaque colors
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            // share the same buffer entry across a number of invocations
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    // skipping over both start and end to get colors
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub const VERTICES: &[Vertex] = &[
    Vertex {
        start: [-1.0, -1.0, 0.0],
        end: [1.0, -1.0, 0.0],
        color: [0.9, 0.9, 0.9],
    }, // top line
    Vertex {
        start: [-1.0, 1.0, 0.0],
        end: [-1.0, -1.0, 0.0],
        color: [0.9, 0.9, 0.9],
    }, // left line
    Vertex {
        start: [1.0, 1.0, 0.0],
        end: [-1.0, 1.0, 0.0],
        color: [0.9, 0.9, 0.9],
    }, // bottom line
    Vertex {
        start: [1.0, -1.0, 0.0],
        end: [1.0, 1.0, 0.0],
        color: [0.9, 0.9, 0.9],
    }, // right line
];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    window_size: [f32; 4],
    max_travels: [f32; 3],
    stroke_width: f32,
}

impl Uniforms {
    pub fn new(window_size: PhysicalSize<u32>, max_travels: &Point) -> Self {
        Self {
            window_size: [
                window_size.width as f32,
                window_size.height as f32,
                0.0,
                0.0,
            ],
            max_travels: [
                max_travels.x() as f32,
                max_travels.y() as f32,
                max_travels.z() as f32,
            ],
            stroke_width: DEFAULT_STROKE_WIDTH,
        }
    }

    pub fn set_max_travels(&mut self, max_travels: &Point) {
        self.max_travels = [
            max_travels.x() as f32,
            max_travels.y() as f32,
            max_travels.z() as f32,
        ];
    }
}
