//! # Stock
//!
//! Creates and manages workpiece stock throughout program execution.
//!
//! Stock is represented as a [`StockTracker`],
//! which exposes [`StockTracker::cut`] for simulating cutting operations on the stock.
//!
//! At a lower level, stock is made up of up to one million of tiny [`StockInstance`]s.
//! Each [`StockInstance`] acts as a little vertical cell (voxel) *with dynamic height*.
//! [`StockTracker`] manipulates this height on a per instance basis to
//! shorten or hide the said instance.
//!
//! ### Terminology
//! Although this module makes repeated use of the term *voxel*,
//! a [`StockInstance`] is **not** an actual *voxel* in the traditional sense.
//! It is **not** a cube but rather a bar placed in the Z direction, whose height can be changed.

use crate::config::{Body, Point, ToolConfig};
use std::f32::consts::SQRT_2;

/// Number of voxels ([`StockInstance`]s) on the longer of X & Y axis.
const STOCK_RESOLUTION: u32 = 1000;

/// Directional bit masks.
const TOP: u32 = 1;
const FRONT: u32 = 1 << 1;
const RIGHT: u32 = 1 << 2;
const BOTTOM: u32 = 1 << 3;
const BACK: u32 = 1 << 4;
const LEFT: u32 = 1 << 5;

// TODO create a relation between distance travelled per frame and stock resolution

/// A single *height-adjustable* voxel instance, with its base at [`Self::center`].
///
/// Each instance has an unsigned integer [`Self::faces`], whose first 6 bits can be used to
/// activate or deactivate faces of the instance in the shader.
/// Instance can be hidden by making the height `0.0`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StockInstance {
    /// X and Y coordinates of the instance base at Z 0.0.
    center: [f32; 2],
    /// Height of the instance to be drawn.
    height: f32,
    /// Visible faces of the voxel instance.
    /// First six bits correspond to **TOP, FRONT, RIGHT, BOTTOM, BACK, LEFT** faces respectively.
    faces: u32,
}

/// A single vertex belonging to a [`StockInstance`].
///
/// To enable face toggling on a per-instance basis,
/// this struct groups each target `face` with all 4 of its vertices.
/// This is done for all 6 faces.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StockInstanceVertex {
    /// X and Y coordinates of the instance vertex.
    xy: [f32; 2],
    /// Z height flag.
    /// To target base(0 height), set to 0.
    /// To target [`StockInstance::height`], set to 1.
    z: u32,
    /// Face the vertex belongs to.
    face: u32,
}

impl StockInstance {
    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`StockInstanceVertex`] is stored in a GPU buffer.
    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<StockInstanceVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: (size_of::<[f32; 2]>() + size_of::<u32>()) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }

    /// Slice of [`StockInstanceVertex`] required to construct all the individual faces of a
    /// [`StockInstance`] independently.
    ///
    /// These faces can be toggled with [`StockInstance::faces`] field.
    /// Z height of the instance is not encoded into this slice and depends on
    /// [`StockInstance::height`].
    pub fn vertices(stock: &StockTracker) -> [StockInstanceVertex; 24] {
        let half_edge = stock.voxel_edge / 2.0;
        [
            // front
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 0 left-near
                z: 0,                         // bottom
                face: FRONT,
            },
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 1 right-near
                z: 0,                        // bottom
                face: FRONT,
            },
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 2 left-near
                z: 1,                         // top
                face: FRONT,
            },
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 3 right-near
                z: 1,                        // top
                face: FRONT,
            },
            // right
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 4 right-near
                z: 0,                        // bottom
                face: RIGHT,
            },
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 5 right-far
                z: 0,                       // bottom
                face: RIGHT,
            },
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 6 right-near
                z: 1,                        // top
                face: RIGHT,
            },
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 7 right-far
                z: 1,                       // top
                face: RIGHT,
            },
            // top
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 8 left-near
                z: 1,                         // top
                face: TOP,
            },
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 9 right-near
                z: 1,                        // top
                face: TOP,
            },
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 10 left-far
                z: 1,                        // top
                face: TOP,
            },
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 11 right-far
                z: 1,                       // top
                face: TOP,
            },
            // back
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 12 right-far
                z: 0,                       // bottom
                face: BACK,
            },
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 13 left-far
                z: 0,                        // bottom
                face: BACK,
            },
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 14 right-far
                z: 1,                       // top
                face: BACK,
            },
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 15 left-far
                z: 1,                        // top
                face: BACK,
            },
            // left
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 16 left-far
                z: 0,                        // bottom
                face: LEFT,
            },
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 17 left-near
                z: 0,                         // bottom
                face: LEFT,
            },
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 18 left-far
                z: 1,                        // top
                face: LEFT,
            },
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 19 left-near
                z: 1,                         // top
                face: LEFT,
            },
            // bottom
            StockInstanceVertex {
                xy: [half_edge, -half_edge], // 20 right-near
                z: 0,                        // bottom
                face: BOTTOM,
            },
            StockInstanceVertex {
                xy: [-half_edge, -half_edge], // 21 left-near
                z: 0,                         // bottom
                face: BOTTOM,
            },
            StockInstanceVertex {
                xy: [half_edge, half_edge], // 22 right-far
                z: 0,                       // bottom
                face: BOTTOM,
            },
            StockInstanceVertex {
                xy: [-half_edge, half_edge], // 23 left-far
                z: 0,                        // bottom
                face: BOTTOM,
            },
        ]
    }

    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`StockInstance`] is stored in a GPU buffer.
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            // this buffer represents unique data across instances
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: (size_of::<[f32; 2]>() + size_of::<u32>()) as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }

    /// Indices of [`Self::vertices`] slice to prevent duplication of vertices for the same face.
    ///
    /// The indices are ordered in **Counter-clockwise** order for each face.
    pub fn indices() -> [u16; 36] {
        [
            2, 0, 1, 1, 3, 2, // front
            6, 4, 5, 5, 7, 6, // right
            10, 8, 9, 9, 11, 10, // top
            14, 12, 13, 13, 15, 14, // back
            18, 16, 17, 17, 19, 18, // left
            22, 20, 21, 21, 23, 22, // bottom
        ]
    }
}

/// Represents a solid cuboidal stock.
/// Tracks the state changes of voxel instances during cutting moves.
#[derive(Debug)]
pub struct StockTracker {
    /// A 2D array of voxels.
    /// Each voxel is represented as a [`StockInstance`].
    instances: Vec<StockInstance>,
    /// Voxel instance counts along X & Y axis.
    voxel_counts: (usize, usize),
    /// Size of the cuboid being represented.
    size: Point,
    /// Total count of voxel instances. Hidden and visible.
    pub total_count: usize,
    /// Index of the first voxel instance whose state changed recently, as a result of
    /// [`Self::cut`], and needs to be reuploaded to the GPU.
    /// Does not have to be index of the first voxel instance that was cut,
    /// and may be the index of its left neighbour.
    start_index: usize,
    /// Index of the last voxel instance whose state changed recently, as a result of [`Self::cut`].
    /// This index, along with [`Self::start_index`], makes drawing of stock changes much more
    /// efficient, as compared to reuploading the whole 2D array of instances.
    /// Does not have to be index of the last voxel instance that was cut,
    /// and may be the index of its right neighbour.
    end_index: usize,
    /// Extent of each voxel instance in X & Y.
    /// Calculated once at generation. Depends on [`Self::size`] and [`STOCK_RESOLUTION`].
    voxel_edge: f32,
}

impl StockTracker {
    /// Creates a new [`StockTracker`], setup to track a cuboid stock.
    /// The supplied body **must** be a [`Body::Cuboid`], as [`Body::Cylinder`] is not implemented yet.
    ///
    /// Calculates the [`Self::voxel_edge`] size to be used for each voxel ([`StockInstance`]).
    /// Only activates the  exposed faces of each [`StockInstance`].
    ///
    /// # Panics
    /// Panics if [`Body::Cylinder`] is provided.
    pub fn new(body: Body) -> Self {
        let size = match body {
            Body::Cuboid { x, y, z } => Point { x, y, z },
            Body::Cylinder { .. } => unreachable!("cylinder not implemented yet"),
        };

        let largest = size.x.max(size.y);
        let edge = largest / STOCK_RESOLUTION as f32; // edge of each bar

        let start = edge / 2.0;

        let mut current_x = start;
        let mut current_y = start;
        let count_x = (size.x / edge).ceil() as usize;
        let count_y = (size.y / edge).ceil() as usize;
        let total_count = count_x * count_y;

        let mut instances = Vec::with_capacity(total_count);

        for x in 0..count_x {
            for y in 0..count_y {
                let mut faces = TOP | BOTTOM;

                if x == 0 {
                    faces |= LEFT
                }

                if x == count_x - 1 {
                    faces |= RIGHT
                }

                if y == 0 {
                    faces |= FRONT
                }

                if y == count_y - 1 {
                    faces |= BACK
                }

                instances.push(StockInstance {
                    center: [current_x, current_y],
                    height: size.z,
                    faces,
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
            start_index: 0,
            end_index: total_count - 1,
            voxel_edge: edge,
        }
    }

    /// Simulates cutting of stock by shortening the voxel instances intersecting with `tool` at
    /// the provided `tool_pos`.
    ///
    /// Returns `true` if at least one voxel instance was shortened,
    /// and `false` to signal no change to instances.
    ///
    /// In case a change is detected, updates [`Self::start_index`] and [`Self::end_index`] to
    /// reflect the first and last of the changed instances. These can be retrieved as a slice of
    /// [`StockInstance`]s with [`Self::instances`].
    pub fn cut(&mut self, tool: ToolConfig, tool_pos: Point) -> bool {
        if tool_pos.z >= self.size.z {
            return false; // tool is not touching the stock
        }

        let rad = tool.diameter / 2.0;

        // create rectangular bounds
        let max_x = tool_pos.x + rad;
        let min_x = tool_pos.x - rad;
        let max_y = tool_pos.y + rad;
        let min_y = tool_pos.y - rad;

        if max_x < 0.0 || max_y < 0.0 || min_x > self.size.x || min_y > self.size.y {
            return false; // tool is not touching the stock
        }

        let edge = self.voxel_edge;
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
        let count_y = self.voxel_counts.1;
        let mut start_index = None;
        let mut end_index = 0;

        for x_index in min_x_index..=max_x_index {
            for y_index in min_y_index..=max_y_index {
                let index = self.voxel_counts.1 * x_index + y_index;

                let target = &mut self.instances[index];

                let dist = ((tool_pos.x - target.center[0]).powi(2)
                    + (tool_pos.y - target.center[1]).powi(2))
                .sqrt();

                if dist > max_dist || target.height <= tool_pos.z {
                    continue;
                }

                // target voxel is higher than the tool and will be shortened
                target.height = if tool_pos.z <= 0.0 {
                    0.0 // the voxel is hidden now
                } else {
                    tool_pos.z
                };

                // check which neighbours have changed
                let neighbours = self.show_neighbours(index, tool_pos.z); // checks for bounds

                // voxel refresh indices depends on the neighbouring voxel too
                if start_index.is_none() {
                    start_index = if neighbours & LEFT != 0 {
                        Some(index - count_y)
                    } else if neighbours & FRONT != 0 {
                        Some(index - 1)
                    } else {
                        Some(index)
                    };
                }

                end_index = if neighbours & RIGHT != 0 {
                    index + count_y
                } else if neighbours & BACK != 0 {
                    index + 1
                } else {
                    index
                };
            }
        }

        match start_index {
            Some(start_index) => {
                self.start_index = start_index;
                self.end_index = end_index;
                true
            }
            None => false, // no voxel change
        }
    }

    /// Returns a continuous slice of all the changed voxel instances from the last call to
    /// [`Self::cut`].
    ///
    /// Also returns the start index of the said slice in relation to the full stock.
    /// This index can be used as the *offset* required to update the GPU instance buffer.
    pub fn instances(&self) -> (usize, &[StockInstance]) {
        debug_assert!(self.start_index <= self.end_index);

        (
            self.start_index, // buffer offset
            &self.instances[self.start_index..=self.end_index],
        )
    }

    /// Reset each voxel instance height back to the same height used at generation.
    pub fn reset(&mut self) {
        self.start_index = 0;
        self.end_index = self.total_count - 1;

        for instance in &mut self.instances {
            instance.height = self.size.z;
        }
    }

    /// Takes a voxel [`StockInstance`] at `index` in the 2D internal array and checks which of its
    /// neighbours will be exposed, if the said instance is lowered to the provided `height`.
    ///
    /// Exposes faces of the neighbours that would become visible and returns the direction of
    /// affected neighbours relative to the instance at `index` by ORing the directional bits.
    ///
    /// Performs bounds check to prevent *index-out-of-range* errors.
    ///
    /// # Panics
    /// Panics if the `index` provided is larger than the total number of voxel instances.
    fn show_neighbours(&mut self, index: usize, height: f32) -> u32 {
        assert!(index < self.total_count, "invalid index");

        let count_y = self.voxel_counts.1;

        let mut sides = 0; // voxels changed on the relative side of current voxel
        if index >= count_y {
            let left = &mut self.instances[index - count_y]; // voxel on left side
            if left.height > height && (left.faces & RIGHT == 0) {
                left.faces |= RIGHT;
                sides |= LEFT;
            }
        }

        if index < self.total_count - count_y {
            let right = &mut self.instances[index + count_y]; // voxel on right side
            if right.height > height && (right.faces & LEFT == 0) {
                right.faces |= LEFT;
                sides |= RIGHT;
            }
        }

        if !index.is_multiple_of(count_y) {
            let front = &mut self.instances[index - 1]; // voxel in the front
            if front.height > height && (front.faces & BACK == 0) {
                front.faces |= BACK;
                sides |= FRONT;
            }
        }

        if !(index + 1).is_multiple_of(count_y) {
            let back = &mut self.instances[index + 1]; // voxel in the back
            if back.height > height && (back.faces & FRONT == 0) {
                back.faces |= FRONT;
                sides |= BACK;
            }
        }

        sides
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a custom stock of low resolution for easier testing.
    fn custom_stock() -> StockTracker {
        StockTracker {
            instances: vec![
                StockInstance {
                    center: [5.0, 5.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | LEFT | FRONT,
                },
                StockInstance {
                    center: [5.0, 15.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | LEFT,
                },
                StockInstance {
                    center: [5.0, 25.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | LEFT | BACK,
                },
                StockInstance {
                    center: [15.0, 5.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | FRONT,
                },
                StockInstance {
                    center: [15.0, 15.0],
                    height: 30.0,
                    faces: TOP | BOTTOM,
                },
                StockInstance {
                    center: [15.0, 25.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | BACK,
                },
                StockInstance {
                    center: [25.0, 5.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | RIGHT | FRONT,
                },
                StockInstance {
                    center: [25.0, 15.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | RIGHT,
                },
                StockInstance {
                    center: [25.0, 25.0],
                    height: 30.0,
                    faces: TOP | BOTTOM | RIGHT | BACK,
                },
            ],
            voxel_counts: (3, 3),
            size: Point::new(30.0, 30.0, 30.0),
            total_count: 9,
            start_index: 0,
            end_index: 8,
            voxel_edge: 10.0,
        }
    }

    #[test]
    fn stock() {
        let size = Point::new(500.0, 250.0, 250.0);
        let edge = size.x.max(size.y) / STOCK_RESOLUTION as f32; // edge of each cube

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
                let mut faces = TOP | BOTTOM;

                if current_x == start {
                    faces |= LEFT
                }

                if current_x + edge > size.x {
                    faces |= RIGHT
                }

                if current_y == start {
                    faces |= FRONT
                }

                if current_y + edge > size.y {
                    faces |= BACK
                }

                oracle.push(StockInstance {
                    center: [current_x, current_y],
                    height: size.z,
                    faces,
                });
                current_y += edge;
            }
            current_y = start;
            current_x += edge;
        }

        let stock_tracker = StockTracker::new(Body::Cuboid {
            x: size.x,
            y: size.y,
            z: size.z,
        });

        assert_eq!(stock_tracker.instances, oracle);
        assert_eq!(stock_tracker.voxel_counts, (count_x, count_y));
        assert_eq!(stock_tracker.voxel_edge, edge);
        assert_eq!(stock_tracker.start_index, 0);
        assert_eq!(stock_tracker.end_index, total_count - 1);
    }

    #[test]
    fn neighbours() {
        let mut stock_tracker = custom_stock();

        // remove the voxel at center
        stock_tracker.show_neighbours(stock_tracker.total_count / 2, 0.0);

        let oracle = vec![
            StockInstance {
                center: [5.0, 5.0],
                height: 30.0,
                faces: TOP | BOTTOM | LEFT | FRONT, // stays the same
            },
            StockInstance {
                center: [5.0, 15.0],
                height: 30.0,
                faces: TOP | BOTTOM | LEFT | RIGHT, // right face must be exposed
            },
            StockInstance {
                center: [5.0, 25.0],
                height: 30.0,
                faces: TOP | BOTTOM | LEFT | BACK, // same
            },
            StockInstance {
                center: [15.0, 5.0],
                height: 30.0,
                faces: TOP | BOTTOM | FRONT | BACK, // back face must be exposed
            },
            StockInstance {
                center: [15.0, 15.0],
                height: 30.0, // not cut, this test focuses on faces
                faces: TOP | BOTTOM,
            },
            StockInstance {
                center: [15.0, 25.0],
                height: 30.0,
                faces: TOP | BOTTOM | BACK | FRONT, // front face must be exposed
            },
            StockInstance {
                center: [25.0, 5.0],
                height: 30.0,
                faces: TOP | BOTTOM | RIGHT | FRONT, // same
            },
            StockInstance {
                center: [25.0, 15.0],
                height: 30.0,
                faces: TOP | BOTTOM | RIGHT | LEFT, // left face must be exposed
            },
            StockInstance {
                center: [25.0, 25.0],
                height: 30.0,
                faces: TOP | BOTTOM | RIGHT | BACK, // same
            },
        ];

        assert_eq!(stock_tracker.instances, oracle);
    }

    #[test]
    fn cut() {
        let mut stock_tracker = StockTracker::new(Body::Cuboid {
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

        let mut instances = Vec::with_capacity(stock_tracker.total_count);

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
                    instances.push(StockInstance {
                        center: [current_x, current_y],
                        height: 0.0,
                        faces: 0, // does not test faces
                    });
                } else {
                    instances.push(StockInstance {
                        center: [current_x, current_y],
                        height: stock_tracker.size.z,
                        faces: 0, // does not test faces
                    });
                }

                current_y += stock_tracker.voxel_edge;
            }
            current_y = start;
            current_x += stock_tracker.voxel_edge;
        }

        assert!(altered);
        for i in 0..stock_tracker.total_count {
            assert_eq!(stock_tracker.instances[i].center, instances[i].center);
            assert_eq!(stock_tracker.instances[i].height, instances[i].height);
            // again, not testing faces here
        }
    }
}
