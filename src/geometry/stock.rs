use crate::config::{Body, Point, ToolConfig};
use std::f32::consts::SQRT_2;

/// number of cubes on the longest axis
const STOCK_RESOLUTION: f32 = 500.0;

// create a relation between distance travelled per frame and stock resolution
#[derive(Debug)]
pub struct Stock {
    instances: Vec<StockInstance>,
    voxel_counts: (usize, usize), // 1 start
    size: Point,
    pub total_count: usize,
    // index of the first voxel that changed recently
    pub cut_start_index: usize,
    // index of the last voxel that changed on last cut call
    pub cut_end_index: usize,
    voxel_edge: f32,
}

impl Stock {
    pub fn new(body: Body) -> Self {
        let size = match body {
            Body::Cuboid { x, y, z } => Point { x, y, z },
            Body::Cylinder { .. } => unreachable!("cylinder not implemented yet"),
        };

        let largest = size.x.max(size.y);
        let edge = largest / STOCK_RESOLUTION; // edge of each bar

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
            cut_start_index: 0,
            cut_end_index: total_count - 1,
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

        // between tool center and voxel center in xy plane
        // consider a 2d voxel square, split the edge and its diagonal is the max
        // possible distance
        let max_dist = rad + (edge / 2.0) * SQRT_2;
        let mut cut_start_index = None;
        let mut cut_end_index = 0;

        for x_index in min_x_index..=max_x_index {
            for y_index in min_y_index..=max_y_index {
                let index = self.voxel_counts.1 * x_index + y_index;

                let target = self
                    .instances
                    .get_mut(index)
                    .expect("stock buffer layout invalid, logic error!");

                let dist = ((pos.x - target.center[0]).powi(2)
                    + (pos.y - target.center[1]).powi(2))
                .sqrt();

                if dist > max_dist || target.height <= pos.z {
                    continue;
                }

                // the voxel is higher than the tool and will be shortened
                if cut_start_index.is_none() {
                    cut_start_index = Some(index);
                    cut_end_index = index // if only a single voxel is cut
                } else {
                    cut_end_index = index
                }

                target.height = if pos.z <= 0.0 {
                    0.0 // the voxel is hidden now
                } else {
                    pos.z
                };
            }
        }

        match cut_start_index {
            Some(cut_start_index) => {
                self.cut_start_index = cut_start_index;
                self.cut_end_index = cut_end_index;
                true
            }
            None => false, // no voxel change
        }
    }

    // returns a continuous slice of all the changed voxel from the last cut
    // shader will check which ones to show
    pub fn instances(&self) -> (usize, &[StockInstance]) {
        debug_assert!(self.cut_start_index <= self.cut_end_index);

        (
            self.cut_start_index, // buffer offset
            &self.instances[self.cut_start_index..=self.cut_end_index],
        )
    }

    pub fn reset(&mut self) {
        self.cut_start_index = 0;
        self.cut_end_index = self.total_count - 1;

        for instance in &mut self.instances {
            instance.height = self.size.z;
        }
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
    pub fn vertices(stock: &Stock) -> [[f32; 3]; 8] {
        let half_edge = stock.voxel_edge / 2.0;
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

        let mut oracle = Vec::with_capacity(count_x * count_y);

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

        let stock_tracker = Stock::new(Body::Cuboid {
            x: size.x,
            y: size.y,
            z: size.z,
        });

        assert_eq!(stock_tracker.instances, oracle);
        assert_eq!(stock_tracker.voxel_counts, (count_x, count_y));
        assert_eq!(stock_tracker.voxel_edge, edge);
    }

    #[test]
    fn cut() {
        let mut stock_tracker = Stock::new(Body::Cuboid {
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
        let max_dist = 25.0 + (stock_tracker.voxel_edge / 2.0) * SQRT_2;

        let mut current_x = start;
        let mut current_y = start;
        while current_x < stock_tracker.size.x {
            while current_y < stock_tracker.size.y {
                if current_y >= 100.0
                    && current_y <= 150.0
                    && current_x >= 225.0
                    && current_x <= 275.0
                    && ((250.0 - current_x).powi(2) + (125.0 - current_y).powi(2)).sqrt() < max_dist
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

        let res: Vec<StockInstance> = stock_tracker
            .instances
            .iter()
            .filter_map(|instance| {
                if instance.height > 0.0 {
                    Some(*instance)
                } else {
                    None
                }
            })
            .collect();

        assert!(altered);
        assert_eq!(res, instances);
    }
}
