//! # Line
//!
//! Creates and manages toolpath simulation throughout program execution.
//!
//! [`LinesTracker`] exposes [`LinesTracker::add`] for simulating a **new move**,
//! represented as a [`MotionSummary`],
//! by breaking it up into small [`LineInstance`]s that can then be drawn by a vertex shader.
//! This is done with the help of [`LineInstances`] abstraction.
//!
//! This splitting and drawing of instances depends on the type of [`MotionSummary`]:
//! - [`MotionSummary::Feed`] & [`MotionSummary::Rapid`] are represented as a single
//!   [`LineInstance`] in their final form.
//! - [`MotionSummary::Arc`] is represented as a collection of [`LineInstance`]s on completion.

use crate::{
    machine::{Arc, CircularDirection, Line, MotionSummary, Plane},
    points::{PlanarPoint, Point},
};
use std::f32::consts::PI;

/// Configures a [`LineInstance`] as a rapid move.
const RAPID: u32 = 0;
/// Configures a [`LineInstance`] as a feed move.
const FEED: u32 = 1;

/// Represents a straight line between two points,
/// that can be drawn to the screen with a vertex shader.
///
/// The vertex shader creates 6 vertices (two triangles) per line instance,
/// to create a line with variable thickness.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInstance {
    /// 3D start point of the line.
    pub start: [f32; 3],

    /// 3D end point of the line.
    pub end: [f32; 3],

    /// Move type of the line, for coloring in the shader.
    pub move_type: u32,
}

impl LineInstance {
    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes the per-vertex
    /// data for each [`LineInstance`].
    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<u32>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Uint32,
            }],
        }
    }

    /// Array of vertices required to construct a single [`LineInstance`].
    ///
    /// These vertices are expanded into a *quad* in the shader, using [`Self::indices`].
    pub fn vertices() -> [u32; 4] {
        [0, 1, 2, 3]
    }

    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`LineInstance`] is stored in a GPU buffer.
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }

    /// Indices of [`Self::vertices`] array to prevent duplication of vertices.
    ///
    /// The indices are ordered in **counter-clockwise** order for each quad.
    pub fn indices() -> [u16; 6] {
        [0, 1, 2, 2, 1, 3]
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// configured as a rapid move.
    pub fn rapid_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            move_type: RAPID,
        }
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// configured as a feed move.
    pub fn feed_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            move_type: FEED,
        }
    }
}

/// Represents an iterator of [`LineInstance`]s based on the geometry type.
///
/// The geometry type is used to determine how the new line instances are added to
/// the GPU `lines_instance_buffer`.
pub enum LineInstances {
    /// A single straight line.
    /// Rendered by adding and updating only one new instance to the GPU buffer, in order to save memory.
    Linear(Box<dyn Iterator<Item = LineInstance>>),

    /// A circular arc, split into a number of small line instances.
    /// Rendered by adding each new instance to the GPU buffer.
    Arc(Box<dyn Iterator<Item = LineInstance>>),
}

impl LineInstances {
    /// Converts a [`MotionSummary`] to the corresponding [`LineInstances`] variant.
    fn new(summary: MotionSummary, tracker: &LinesTracker) -> Self {
        match summary {
            MotionSummary::Rapid(line) => {
                Self::linear_points(line, tracker, LineInstance::rapid_move)
            }
            MotionSummary::Feed(line) => {
                Self::linear_points(line, tracker, LineInstance::feed_move)
            }
            MotionSummary::Arc(arc) => Self::arc_points(arc, tracker),
        }
    }

    /// Splits a [`Line`] into a [`LineInstances::Linear`] iterator,
    /// advancing [`tracker::resolution`](LinesTracker::resolution) units per instance
    /// from [`Line::start`] to [`Line::end`].
    ///
    /// See [`LinesTracker::resolution`] for the importance of this constraint.
    ///
    /// Each new instance is rooted at `start` rather than the `end` of the previous line instance.
    ///
    /// The returned iterator is guaranteed to **NOT be empty**, and will return only a single instance
    /// if the length of [`Line`] is shorter than [`tracker::resolution`](LinesTracker::resolution).
    fn linear_points(
        line: Line,
        tracker: &LinesTracker,
        get_instance: fn(Point, Point) -> LineInstance,
    ) -> Self {
        let start = line.start;
        let end = line.end;

        // direction from start to end
        let dir = end - start;
        // distance between start and end points
        let dist = (dir.x.powi(2) + dir.y.powi(2) + dir.z.powi(2)).sqrt();

        if dist <= tracker.resolution {
            return Self::Linear(Box::new([get_instance(start, end)].into_iter()));
        }

        // amount to move each axis by to get next point
        let delta = (dir * tracker.resolution) / dist;

        let mut current = start;

        Self::Linear(Box::new(std::iter::from_fn(move || {
            if current == end {
                return None;
            }

            let next = current + delta;
            let remaining = end - next;

            // use dot product to see if the next point is between start and end
            if remaining.x * dir.x + remaining.y * dir.y + remaining.z * dir.z <= 0.0 {
                current = end;
            } else {
                current = next;
            }

            Some(get_instance(start, current))
        })))
    }

    /// Splits an [`Arc`] into a [`LineInstances::Arc`] iterator,
    /// advancing [`tracker::resolution`](LinesTracker::resolution)
    /// per radius radians per instance from [`Arc::start`] to [`Arc::end`].
    ///
    /// See [`LinesTracker::resolution`] for the importance of this constraint.
    ///
    /// Each new instance starts at the `end` of the previous line instance.
    ///
    /// The returned iterator is guaranteed to **NOT be empty**, and will return only a single instance,
    /// if the angular sweep of [`Arc`] is shorter than [`tracker::resolution`](LinesTracker::resolution)
    /// per radius radians.
    ///
    /// ## Reference
    /// [FreeMathHelp](https://www.freemathhelp.com/forum/threads/xy-points-on-an-arc.130791/)
    fn arc_points(arc: Arc, tracker: &LinesTracker) -> Self {
        let plane = arc.center.plane;
        let start = PlanarPoint::from_point(arc.start, plane);

        let center = arc.center;
        let radius = arc.radius;
        let sweep = arc.sweep;

        // angular speed
        let step_angular = match arc.dir {
            CircularDirection::Clockwise => 0.0 - tracker.resolution / radius / PI,
            CircularDirection::CounterClockwise => tracker.resolution / radius / PI,
        };
        let steps_count = (sweep / step_angular).ceil().abs();
        let step_linear = match plane {
            Plane::XY => arc.end.z - arc.start.z,
            Plane::XZ => arc.end.y - arc.start.y,
            Plane::YZ => arc.end.x - arc.start.x,
        } / steps_count; // amount to move the third axis for each step

        if sweep.abs() <= step_angular.abs() {
            return Self::Arc(Box::new(
                [LineInstance::feed_move(arc.start, arc.end)].into_iter(),
            ));
        }

        // start point relative to arc center
        let rel_start = start - center;

        // minor arc sweep angle with primary axis of the plane in radians
        let mut current_sweep = (rel_start.first / radius).clamp(-1.0, 1.0).acos();
        if rel_start.second.is_sign_negative() {
            current_sweep += PI;
        }
        let mut current_pos = arc.start;
        // total sweep from positive major axis to get to end point
        let end_sweep = current_sweep + sweep;

        Self::Arc(Box::new(std::iter::from_fn(move || {
            // both are exact same on bit level because of direct assignment
            if current_sweep == end_sweep {
                return None;
            }

            current_sweep = match arc.dir {
                CircularDirection::Clockwise => {
                    if current_sweep + step_angular < end_sweep {
                        end_sweep
                    } else {
                        current_sweep + step_angular
                    }
                }
                CircularDirection::CounterClockwise => {
                    if current_sweep + step_angular > end_sweep {
                        end_sweep
                    } else {
                        current_sweep + step_angular
                    }
                }
            };

            // relative to center
            let new_pos = if current_sweep == end_sweep {
                arc.end
            } else {
                match plane {
                    Plane::XY => Point::new(
                        arc.center.first + radius * current_sweep.cos(),
                        arc.center.second + radius * current_sweep.sin(),
                        current_pos.z + step_linear,
                    ),
                    Plane::XZ => Point::new(
                        arc.center.first + radius * current_sweep.cos(),
                        current_pos.y + step_linear,
                        arc.center.second + radius * current_sweep.sin(),
                    ),
                    Plane::YZ => Point::new(
                        current_pos.x + step_linear,
                        arc.center.first + radius * current_sweep.cos(),
                        arc.center.second + radius * current_sweep.sin(),
                    ),
                }
            };

            let ret = Some(LineInstance::feed_move(current_pos, new_pos));

            current_pos = new_pos;

            ret
        })))
    }
}

/// Represents how a [`LineInstance`] should be added to the GPU [`buffer`](wgpu::Buffer),
/// if there is one.
///
/// This type is helpful in differentiating between `linear` and `arc` moves,
/// as well as to prevent the program from feeling sluggish by providing `render` flag,
/// which tells the render loop when the program requires rendering new frame.
pub enum BufferAction {
    Overwrite {
        /// Overwrite the last instance inside the buffer with this new one.
        instance: LineInstance,
        /// Render if the last frame render happened more than `LinesTracker::resolution` units of travel ago.
        render: bool,
    },

    Add {
        /// Add this new instance to the buffer.
        instance: LineInstance,
        /// Render if the last frame render happened more than `LinesTracker::resolution` units of travel ago.
        render: bool,
    },
}

/// Tracks the total length of individual [`LineInstance`]s left to be rendered since last frame render,
/// throughout the program life.
///
/// This is **extremely** useful for *adaptive* toolpaths,
/// that generate hundreds of really small line segments and without total length tracking,
/// rendering a new frame for each of those line segments makes the simulation feel slow and stuttery.
pub struct LinesTracker {
    /// Iterator for [`LineInstance`]s.
    instances: Option<LineInstances>,

    /// Sum of lengths of each [`LineInstance`] from [`Self::instances`] since the last render.
    /// These are the instances that are added to the vertex buffer but not drawn to the surface yet.
    len: f32,

    /// Resolution for splitting [`MotionSummary`] into [`LineInstance`]s.
    ///
    /// This makes sure that the instances are not **too fine or not too blocky**,
    /// helping us reduce instance count and not making the stock cutting simulation feel blocky.
    ///
    /// This is because stock is updated based on the position of the latest line instance.
    resolution: f32,

    /// Flag to check first call to [`Self::next`] after every [`Self::add`] call.
    first: bool,
}

impl LinesTracker {
    /// Construct a new [`LinesTracker`] and configures it to yield [`LineInstance`]s at
    /// double the resolution of the provided `voxel_edge`.
    pub fn new(voxel_edge: f32) -> Self {
        Self {
            instances: None,
            len: 0.0,
            resolution: voxel_edge * 2.0, // won't notice this of a difference
            first: true,
        }
    }

    /// Loads in a new [`LineInstances`].
    /// Any previous instances must be drained before this as those will be lost.
    pub fn add(&mut self, summary: MotionSummary) {
        self.instances = Some(LineInstances::new(summary, self));
        self.first = true;
    }

    /// Resets the internal state of `self`.
    /// Any previous `instances` and sum of instance lengths is lost.
    pub fn reset(&mut self) {
        self.instances = None;
        self.len = 0.0;
        self.first = true;
    }
}

impl Iterator for LinesTracker {
    type Item = BufferAction;

    /// Iterates the stored [`LineInstance`]s and returns a [`BufferAction`] depending on state of `self`.
    ///
    /// - After [`Self::add`] the first call always returns [`BufferAction::Add`],
    ///   this is checked with an internal flag.
    /// - Subsequent calls return [`BufferAction::Overwrite`],
    ///   if stored instances are [`LineInstances::Linear`].
    /// - Subsequent calls return [`BufferAction::Add`],
    ///   if stored instances are [`LineInstances::Arc`].
    /// - Exhaustion of instances returns [`None`].
    ///
    /// In case of [`BufferAction::Overwrite`] & [`BufferAction::Add`],
    /// `render` flag is determined on comparing the sum of each returned [`LineInstance`] since the
    /// last render with the internal resolution for splitting a [`MotionSummary`].
    fn next(&mut self) -> Option<Self::Item> {
        let instance = match self.instances.as_mut()? {
            LineInstances::Linear(lines) => lines.next(),
            LineInstances::Arc(lines) => lines.next(),
        }?;

        self.len += ((instance.end[0] - instance.start[0]).powi(2)
            + (instance.end[1] - instance.start[1]).powi(2)
            + (instance.end[2] - instance.start[2]).powi(2))
        .sqrt();

        // render if the len is now more than acceptable difference between two frames
        let render = self.len >= self.resolution;
        if render {
            self.len = 0.0;
        }

        let ret = match self.instances.as_ref().unwrap() {
            LineInstances::Linear(_) if self.first => BufferAction::Add { instance, render },
            LineInstances::Linear(_) => BufferAction::Overwrite { instance, render },
            LineInstances::Arc(_) => BufferAction::Add { instance, render },
        };

        self.first = false;

        Some(ret)
    }
}
