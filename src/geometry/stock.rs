use crate::config::{Point, Stock, ToolConfig};

/// number of cubes on the longest axis
const STOCK_RESOLUTION: f32 = 100.0;

// create a relation between distance travelled per frame and stock resolution
pub struct StockTracker {
    instances: Vec<StockInstance>,
    voxel_counts: (usize, usize, usize),
    size: Point,
    total_count: usize,
    hidden: usize,
    voxel_edge: f32,
}

impl StockTracker {
    pub fn new(stock: Stock) -> Self {
        let size = match stock {
            Stock::Cuboid { x, y, z } => Point { x, y, z },
            Stock::Cylinder { .. } => unreachable!("cylinder not implemented yet"),
        };

        let largest = size.x.max(size.y).max(size.z);
        let edge = largest / STOCK_RESOLUTION; // edge of each cube

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let mut current_z = start;
        let count_x = (size.x / edge).ceil() as usize;
        let count_y = (size.y / edge).ceil() as usize;
        let count_z = (size.z / edge).ceil() as usize;
        let total_count = count_x * count_y * count_z;

        let mut instances = Vec::with_capacity(total_count);

        for _ in 0..count_x {
            for _ in 0..count_y {
                for _ in 0..count_z {
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
            instances,
            voxel_counts: (count_x, count_y, count_z),
            size,
            total_count,
            hidden: 0,
            voxel_edge: edge,
        }
    }

    fn hide(&mut self, tool: ToolConfig, pos: Point) -> bool {
        if pos.z >= self.size.z {
            return false; // tool is not touching the stock
        }

        let rad = tool.diameter / 2.0;
        let max_x = pos.x + rad;
        let min_x = pos.x - rad;
        let max_y = pos.y + rad;
        let min_y = pos.y - rad;

        if max_x < 0.0 || max_y < 0.0 || min_x > self.size.x || min_y > self.size.y {
            return false; // tool is not touching the stock
        }

        let min_x_index = if min_x < 0.0 {
            0
        } else {
            (min_x / self.voxel_edge).floor() as usize
        };
        let min_y_index = if min_y < 0.0 {
            0
        } else {
            (min_y / self.voxel_edge).floor() as usize
        };
        let min_z_index = if pos.z < 0.0 {
            0
        } else {
            (pos.z / self.voxel_edge).floor() as usize
        };

        let max_x_index = if max_x > self.size.x {
            self.voxel_counts.0
        } else {
            (max_x / self.voxel_edge).ceil() as usize
        };
        let max_y_index = if max_y > self.size.y {
            self.voxel_counts.1
        } else {
            (max_y / self.voxel_edge).ceil() as usize
        };
        let max_z_index = self.voxel_counts.2;

        let mut hidden = 0;

        for x in min_x_index..=max_x_index {
            for y in min_y_index..=max_y_index {
                unsafe {
                    let targets = self
                        .instances
                        .get_unchecked_mut((x * y + min_z_index)..=(x * y + max_z_index));

                    targets.iter_mut().for_each(|target| {
                        if target.visible == 1 {
                            target.visible = 0;
                            hidden += 1;
                        }
                    });
                }
            }
        }

        self.hidden += hidden;

        if hidden > 0 { true } else { false }
    }

    pub fn instances(&self) -> Vec<StockInstance> {
        let mut ret = Vec::with_capacity(self.total_count - self.hidden);

        ret.extend(
            self.instances
                .iter()
                .filter(|instance| instance.visible == 1),
        );

        ret
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
        let count_x = (size.x / edge).ceil() as usize;
        let count_y = (size.y / edge).ceil() as usize;
        let count_z = (size.z / edge).ceil() as usize;
        let total_count = count_x * count_y * count_z;

        let mut oracle = Vec::with_capacity(total_count);

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

        let stock_tracker = StockTracker::new(Stock::Cuboid {
            x: size.x,
            y: size.y,
            z: size.z,
        });

        assert_eq!(stock_tracker.instances, oracle);
        assert_eq!(stock_tracker.voxel_counts, (count_x, count_y, count_z));
        assert_eq!(stock_tracker.total_count, total_count);
        assert_eq!(stock_tracker.voxel_edge, edge);
    }

    #[test]
    fn hide() {
        let mut stock_tracker = StockTracker::new(Stock::Cuboid {
            x: 500.0,
            y: 250.0,
            z: 250.0,
        });

        let tool = ToolConfig {
            number: 1,
            diameter: 50.0,
            length: 50.0,
        };

        let altered = stock_tracker.hide(tool, Point::new(250.0, 125.0, 0.0));

        let mut instances = Vec::new();

        let start = stock_tracker.voxel_edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let mut current_z = start;
        while current_x < stock_tracker.size.x {
            if current_x >= 225.0 && current_x <= 275.0 {
                continue;
            }
            while current_y < stock_tracker.size.y {
                if current_y >= 100.0 && current_y <= 150.0 {
                    continue;
                }
                while current_z < stock_tracker.size.z {
                    instances.push(StockInstance {
                        center: [current_x, current_y, current_z],
                        visible: 1,
                    });
                    current_z += stock_tracker.voxel_edge;
                }
                current_z = start;
                current_y += stock_tracker.voxel_edge;
            }
            current_y = start;
            current_x += stock_tracker.voxel_edge;
        }

        assert!(altered);
        assert_eq!(stock_tracker.instances, instances);
    }
}
