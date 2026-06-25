use crate::config::{Point, Stock};

/// number of cubes on the longest axis
const STOCK_RESOLUTION: f32 = 100.0;

pub struct StockTracker {
    pub size: Point,
    pub voxel_edge: f32,
    pub instances: Vec<StockInstance>,
}

impl StockTracker {
    pub fn new(stock: Stock) -> Self {
        let size = match stock {
            Stock::Cuboid { x, y, z } => Point { x, y, z },
            Stock::Cylinder { .. } => unreachable!("panics in lib"),
        };

        let largest = size.x.max(size.y).max(size.z);
        let edge = largest / STOCK_RESOLUTION; // edge of each cube

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let mut current_z = start;
        let len_x = (size.x / edge).ceil() as usize;
        let len_y = (size.y / edge).ceil() as usize;
        let len_z = (size.z / edge).ceil() as usize;

        let mut instances = Vec::with_capacity(len_x * len_y * len_z);

        for _ in 0..len_x {
            for _ in 0..len_y {
                for _ in 0..len_z {
                    instances.push(StockInstance {
                        center: [current_x, current_y, current_z],
                        visible: 1,
                    });
                    current_z += edge;
                }
                current_z = start;
                current_y += edge;
            }
            current_y = start;
            current_x += edge;
        }

        Self {
            size,
            voxel_edge: edge,
            instances,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StockInstance {
    pub center: [f32; 3],
    pub visible: u32,
}

impl StockInstance {
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

    // vertices of a cube
    // one cube per instance
    pub fn vertices(edge: f32) -> [[f32; 3]; 8] {
        let half_edge = edge / 2.0;
        [
            [-half_edge, -half_edge, -half_edge], // 0 left-bottom-near
            [half_edge, -half_edge, -half_edge],  // 1 right-bottom-near
            [-half_edge, half_edge, -half_edge],  // 2 left-top-near
            [half_edge, half_edge, -half_edge],   // 3 right-top-near
            [-half_edge, -half_edge, half_edge],  // 4 left-bottom-far
            [half_edge, -half_edge, half_edge],   // 5 right-bottom-far
            [-half_edge, half_edge, half_edge],   // 6 left-top-far
            [half_edge, half_edge, half_edge],    // 7 right-top-far
        ]
    }

    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            // this buffer represents unique data across instances
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }

    pub fn indices() -> [u16; 36] {
        [
            2, 0, 1, 1, 3, 2, // front face
            6, 2, 3, 3, 7, 6, // top face
            0, 4, 5, 5, 1, 0, // bottom face
            7, 5, 4, 4, 6, 7, // back face
            6, 4, 0, 0, 2, 6, // left face
            3, 1, 5, 5, 7, 3, // right face
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock() {
        let size = Point::new(500.0, 250.0, 250.0);
        let edge = size.x.max(size.y).max(size.z) / STOCK_RESOLUTION; // edge of each cube

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let mut current_z = start;

        let mut oracle = Vec::with_capacity(
            (size.x / edge).ceil() as usize
                + (size.y / edge).ceil() as usize
                + (size.z / edge).ceil() as usize,
        );

        // slower version, but better reasoning
        while current_x < size.x {
            while current_y < size.y {
                while current_z < size.z {
                    oracle.push(StockInstance {
                        center: [current_x, current_y, current_z],
                        visible: 1,
                    });
                    current_z += edge;
                }
                current_z = start;
                current_y += edge;
            }
            current_y = start;
            current_x += edge;
        }

        let stock = Stock::new(size);

        assert_eq!(stock.instances, oracle);
        assert_eq!(stock.voxel_edge, edge);
    }
}
