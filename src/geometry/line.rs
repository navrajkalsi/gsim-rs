use std::f32::consts::PI;

use crate::{
    config::Point,
    machine::{Arc, CircularDirection, Line, MotionSummary, PlanarPoint},
    parser::Plane,
};

const RAPID_MOVE: u32 = 0;
const FEED_MOVE: u32 = 1;

/// Machine units travelled per frame.
const SPEED: f32 = 5.0;

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

    // vertices of a quad
    // one quad per instance
    pub fn vertices() -> [u32; 4] {
        [0, 1, 2, 3]
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

    pub fn indices() -> [u16; 6] {
        [0, 1, 2, 2, 1, 3] // counter clockwise
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// with [`STROKE_WIDTH`] and [`RAPID_MOVE_COLOR`].
    pub fn rapid_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            move_type: RAPID_MOVE,
        }
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// with [`STROKE_WIDTH`] and [`FEED_MOVE_COLOR`].
    pub fn feed_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            move_type: FEED_MOVE,
        }
    }
}

/// Represents an iterator of [`LineInstance`]s based on the geometry type.
///
/// The geometry type is used to determine how the new line instances are added to
/// the GPU [`buffer`](crate::gui::Graphics::lines_buffer);
enum LineInstances {
    /// A single straight line.
    /// Rendered by adding and updating only one new instance to the GPU buffer, in order to save memory.
    Linear(Box<dyn Iterator<Item = LineInstance>>),
    /// A circular arc, split into a number of small line instances.
    /// Rendered by adding each new instance to the GPU buffer.
    Arc(Box<dyn Iterator<Item = LineInstance>>),
}

impl LineInstances {
    /// Converts a [`MotionSummary`] to the corresponding [`LineInstances`] variant.
    fn new(summary: MotionSummary) -> Self {
        match summary {
            MotionSummary::Rapid(line) => Self::linear_points(line, LineInstance::rapid_move),
            MotionSummary::Feed(line) => Self::linear_points(line, LineInstance::feed_move),
            MotionSummary::Arc(arc) => Self::arc_points(arc),
        }
    }

    /// Splits a [`Line`] into a [`LineInstances::Linear`] iterator,
    /// advancing [`SPEED`] units per instance from [`Line::start`] to [`Line::end`].
    ///
    /// Each new instance is rooted at `start` rather than the `end` of the previous line instance.
    ///
    /// The returned iterator is guaranteed to **NOT be empty**, and will return only a single instance,
    /// if the length of [`Line`] is shorter than [`SPEED`].
    fn linear_points(line: Line, get_instance: fn(Point, Point) -> LineInstance) -> Self {
        let start = line.start;
        let end = line.end;

        // direction from start to end
        let dir = end - start;
        // distance between start and end points
        let dist = (dir.x.powi(2) + dir.y.powi(2) + dir.z.powi(2)).sqrt();

        if dist <= SPEED {
            return Self::Linear(Box::new([get_instance(start, end)].into_iter()));
        }

        // amount to move each axis by to get next point
        let delta = dir.mul_float(SPEED).div_float(dist);

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
    /// advancing [`SPEED`] per radius radians per instance from [`Arc::start`] to [`Arc::end`].
    ///
    /// Each new instance starts at the `end` of the previous line instance.
    ///
    /// The returned iterator is guaranteed to **NOT be empty**, and will return only a single instance,
    /// if the angular sweep of [`Arc`] is shorter than [`SPEED`] per arc radius.
    ///
    /// ## Reference
    /// [FreeMathHelp](https://www.freemathhelp.com/forum/threads/xy-points-on-an-arc.130791/)
    fn arc_points(arc: Arc) -> Self {
        let plane = arc.center.plane();
        let start = PlanarPoint::from_point(arc.start, plane);

        let center = arc.center;
        let radius = arc.radius;
        let sweep = arc.sweep;

        // angular speed
        let step_angular = match arc.dir {
            CircularDirection::Clockwise => 0.0 - SPEED / radius / 2.0,
            CircularDirection::CounterClockwise => SPEED / radius / 2.0,
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
        let mut current_sweep = (rel_start.first() / radius).clamp(-1.0, 1.0).acos();
        if rel_start.second().is_sign_negative() {
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
                        arc.center.first() + radius * current_sweep.cos(),
                        arc.center.second() + radius * current_sweep.sin(),
                        current_pos.z + step_linear,
                    ),
                    Plane::XZ => Point::new(
                        arc.center.first() + radius * current_sweep.cos(),
                        current_pos.y + step_linear,
                        arc.center.second() + radius * current_sweep.sin(),
                    ),
                    Plane::YZ => Point::new(
                        current_pos.x + step_linear,
                        arc.center.first() + radius * current_sweep.cos(),
                        arc.center.second() + radius * current_sweep.sin(),
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
/// as well as to prevent the program from feeling sluggish by providing `render` flag.
pub enum BufferAction {
    /// Overwrite the last instance inside the buffer with a new one.
    /// Render if the last frame render happened more than [`SPEED`] units of travel ago.
    Overwrite {
        instance: LineInstance,
        render: bool,
    },
    /// Add a new instance to the buffer.
    /// Render if the last frame render happened more than [`SPEED`] units of travel ago.
    Add {
        instance: LineInstance,
        render: bool,
    },
    /// No new instance available.
    /// Use [`LineInstancesTracker::add`] to add new [`MotionSummary`].
    Exhausted,
}

/// Tracks the total length of individual [`LineInstance`]s left to be rendered since last frame render,
/// throughout the program life.
///
/// This is **extremely** useful for *adaptive* toolpaths,
/// that generate hundreds of really small line segments and without total length tracking,
/// rendering a new frame for each of those line segments makes the simulation feel slow and stuttery.
pub struct LineInstancesTracker {
    /// Iterator for [`LineInstance`]s.
    instances: Option<LineInstances>,
    /// Sum of lenghts of each [`LineInstance`] from [`Self::instances`] since the last render.
    /// These are the instances that are added to the vertex buffer but not drawn to the surface yet.
    len: f32,
    /// Flag to check first call to [`Self::next`] after every [`Self::add`] call.
    first: bool,
}

impl LineInstancesTracker {
    /// Construct a new [`LineInstancesTracker`].
    pub fn new() -> Self {
        Self {
            instances: None,
            len: 0.0,
            first: true,
        }
    }

    /// Loads a new [`LineInstances`] into [`Self::instances`] and sets [`Self::first`].
    /// Expects the previous `instances` to be [`None`].
    ///
    /// # Panics
    /// Panics if previous `instances` have not been drained yet.
    pub fn add(&mut self, summary: MotionSummary) {
        if self.instances.is_some() {
            unreachable!("Previous instances not exhausted.");
        }

        self.instances = Some(LineInstances::new(summary));
        self.first = true;
    }

    /// Iterates [`Self::instances`] and returns a [`BufferAction`] depending on state of `self`.
    ///
    /// - First call always returns [`BufferAction::Add`], this is checked with [`Self::first`] flag.
    /// - Subsequent calls return [`BufferAction::Overwrite`],
    ///   if [`Self::instances`] is [`LineInstances::Linear`].
    /// - Subsequent calls return [`BufferAction::Add`],
    ///   if [`Self::instances`] is [`LineInstances::Arc`].
    /// - Exhaustion of instances returns [`BufferAction::Exhausted`].
    ///
    /// In case of [`BufferAction::Overwrite`] & [`BufferAction::Add`],
    /// `render` flags is determined on comparing [`Self::len`] with [`SPEED`].
    ///
    /// # Panics
    /// Panics if called when [`Self::instances`] is [`None`].
    // TODO
    pub fn next(&mut self) -> BufferAction {
        let instances = match self.instances.as_mut() {
            Some(LineInstances::Linear(lines)) => lines.next(),
            Some(LineInstances::Arc(lines)) => lines.next(),
            None => return BufferAction::Exhausted,
        };

        let instance = match instances {
            Some(line) => line,
            None if self.first => {
                unreachable!("At least one point is guarranteed, which would be the end point.")
            }
            None => {
                self.instances = None;
                return BufferAction::Exhausted;
            }
        };

        self.len += ((instance.end[0] - instance.start[0]).powi(2)
            + (instance.end[1] - instance.start[1]).powi(2)
            + (instance.end[2] - instance.start[2]).powi(2))
        .sqrt();

        // render if the len is now more than acceptable difference between two frames
        let render = self.len >= SPEED;
        if render {
            self.len = 0.0;
        }

        let ret = match self.instances.as_ref().unwrap() {
            LineInstances::Linear(_) if self.first => BufferAction::Add { instance, render },
            LineInstances::Linear(_) => BufferAction::Overwrite { instance, render },
            LineInstances::Arc(_) => BufferAction::Add { instance, render },
        };

        self.first = false;

        ret
    }

    /// Resets the internal state of `self`.
    ///
    /// Any previous `instances` and sum of instance lengths is lost.
    pub fn reset(&mut self) {
        self.instances = None;
        self.len = 0.0;
        self.first = true;
    }
}
