use crate::config::{Point, Stock, ToolConfig};
use std::f32::consts::SQRT_2;

/// number of cubes on the longest axis
const STOCK_RESOLUTION: f32 = 1000.0;

// create a relation between distance travelled per frame and stock resolution
#[derive(Debug)]
pub struct StockTracker {
    instances: Vec<StockInstance>,
    voxel_counts: (usize, usize), // not zero indexed
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

        let largest = size.x.max(size.y);
        let edge = largest / STOCK_RESOLUTION; // edge of each cube

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let count_x = (size.x / edge).ceil() as usize;
        let count_y = (size.y / edge).ceil() as usize;
        let total_count = count_x * count_y;

        let mut instances = Vec::with_capacity(total_count);

        for _ in 0..count_x {
            for _ in 0..count_y {
                instances.push(StockInstance {
                    center: [current_x, current_y],
                    height: size.z,
                });
                current_y += edge;
            }
            current_y = start;
            current_x += edge;
        }

        Self {
            instances,
            voxel_counts: (count_x, count_y),
            size,
            total_count,
            hidden: 0,
            voxel_edge: edge,
        }
    }

    pub fn cut(&mut self, tool: ToolConfig, pos: Point) -> bool {
        if pos.z >= self.size.z {
            return false; // tool is not touching the stock
        }

        let rad = tool.diameter / 2.0;
        // create rectangular bounds
        let max_x = pos.x + rad;
        let min_x = pos.x - rad;
        let max_y = pos.y + rad;
        let min_y = pos.y - rad;

        if max_x < 0.0 || max_y < 0.0 || min_x > self.size.x || min_y > self.size.y {
            return false; // tool is not touching the stock
        }

        let edge = self.voxel_edge;
        // these indices do not consider z axis
        let min_x_index = if min_x < 0.0 {
            0
        } else {
            (min_x / edge).floor() as usize
        };
        let min_y_index = if min_y < 0.0 {
            0
        } else {
            (min_y / edge).floor() as usize
        };

        let max_x_index = if max_x > self.size.x {
            self.voxel_counts.0 - 1 // counts are not 0 indexed
        } else {
            let ret = max_x / self.voxel_edge;
            let floored = ret.floor();
            if ret - floored > 0.1 {
                floored as usize // beyond boundary, hide this cell
            } else {
                floored as usize - 1 // on the boundary, hide previous cell
            }
        };
        let max_y_index = if max_y > self.size.y {
            self.voxel_counts.1 - 1
        } else {
            let ret = max_y / self.voxel_edge;
            let floored = ret.floor();
            if ret - floored > 0.1 {
                floored as usize // beyond boundary, hide this cell
            } else {
                floored as usize - 1 // on the boundary, hide previous cell
            }
        };

        let mut hidden = 0;
        let mut cut = false;

        for x_index in min_x_index..=max_x_index {
            for y_index in min_y_index..=max_y_index {
                let index = self.voxel_counts.1 * x_index + y_index;

                let target = self
                    .instances
                    .get_mut(index)
                    .expect("stock buffer layout invalid, logic error!");

                // between tool center and voxel center in xy plane
                // consider a 2d voxel square, split the edge and its diagonal is the max
                // possible distance
                let max_dist = rad + (edge / 2.0) * SQRT_2;
                let dist = ((pos.x - target.center[0]).powi(2)
                    + (pos.y - target.center[1]).powi(2))
                .sqrt();

                if dist > max_dist || target.height < pos.z {
                    continue;
                }

                // the voxel is higher than the tool and will be shortened
                cut = true;

                if pos.z <= 0.0 {
                    // the voxel is hidden now
                    hidden += 1;
                    target.height = 0.0;
                } else {
                    target.height = pos.z;
                }
            }
        }

        self.hidden += hidden;

        cut
    }

    pub fn instances(&self) -> &Vec<StockInstance> {
        &self.instances
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StockInstance {
    pub center: [f32; 2],
    pub height: f32,
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

    // vertices of a voxel
    // one cube per instance
    // z of the voxel depends on its final height, which may be less than the stock height
    pub fn vertices(edge: f32) -> [[f32; 3]; 8] {
        let half_edge = edge / 2.0;
        [
            [-half_edge, -half_edge, 0.0], // 0 left-near-bottom
            [half_edge, -half_edge, 0.0],  // 1 rigth-near-bottom
            [-half_edge, -half_edge, 1.0], // 2 left-near-top
            [half_edge, -half_edge, 1.0],  // 3 rigth-near-top
            [-half_edge, half_edge, 0.0],  // 4 left-far-bottom
            [half_edge, half_edge, 0.0],   // 5 right-far-bottom
            [-half_edge, half_edge, 1.0],  // 6 left-far-top
            [half_edge, half_edge, 1.0],   // 7 right-far-top
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
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }

    pub fn indices() -> [u16; 36] {
        [
            2, 0, 1, 1, 3, 2, // front face
            3, 1, 5, 5, 7, 3, // right face
            7, 5, 4, 4, 6, 7, // back face
            6, 4, 0, 0, 2, 6, // left face
            6, 2, 3, 3, 7, 6, // top face
            0, 4, 5, 5, 1, 0, // bottom face
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock() {
        let size = Point::new(500.0, 250.0, 250.0);
        let edge = size.x.max(size.y) / STOCK_RESOLUTION; // edge of each cube

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let count_x = (size.x / edge).ceil() as usize;
        let count_y = (size.y / edge).ceil() as usize;
        let total_count = count_x * count_y;

        let mut oracle = Vec::with_capacity(total_count);

        // slower version, but better reasoning
        while current_x < size.x {
            while current_y < size.y {
                oracle.push(StockInstance {
                    center: [current_x, current_y],
                    height: size.z,
                });
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
        assert_eq!(stock_tracker.voxel_counts, (count_x, count_y));
        assert_eq!(stock_tracker.total_count, total_count);
        assert_eq!(stock_tracker.voxel_edge, edge);
    }

    #[test]
    fn cut() {
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

        let altered = stock_tracker.cut(tool, Point::new(250.0, 125.0, 0.0));

        let mut instances = Vec::new();

        let start = stock_tracker.voxel_edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        while current_x < stock_tracker.size.x {
            while current_y < stock_tracker.size.y {
                if current_y >= 100.0
                    && current_y <= 150.0
                    && current_x >= 225.0
                    && current_x <= 275.0
                    && ((250.0 - current_x).powi(2) + (125.0 - current_y).powi(2)).sqrt()
                        < 25.0 + start
                {
                    current_y += stock_tracker.voxel_edge;
                    continue;
                }
                instances.push(StockInstance {
                    center: [current_x, current_y],
                    height: 250.0,
                });
                current_y += stock_tracker.voxel_edge;
            }
            current_y = start;
            current_x += stock_tracker.voxel_edge;
        }

        assert!(altered);
        assert_eq!(stock_tracker.instances(), instances);
    }
}
