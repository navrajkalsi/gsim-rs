//! # Geometry
//!
//! Constructs [`LineInstance`]s, [`ToolInstance`], and [`Uniforms`] values,
//! to be uploaded to the vertex shader for simulation.
//!
//! [`LineInstance`]s can be generated using [`LineInstancesTracker`],
//! which tracks when the line instances should be drawn to the surface.
//!
//! [`LineInstance`]s depict toolpaths and static view objects,
//! toggled by [`StaticConfig`]:
//! - Machine Boundary Box
//! - XY Plane Grid
//! - Axis Pointers, rooted at Origin.
//!
//! ## Depth
//! Each instance has a depth value that corresponds to the **z value in NDC**,
//! in the vertex shader. `0.0` is choosen as the nearest plane, and `1.0` as the farthest.
//! Instances are laid out in the following order of increasing depth:
//! - Tool
//! - Machine boundary
//! - Origin axes
//! - Tool paths
//! - Grid
//!
//! ## Important
//! Assumes that all the [`Machine`](crate::machine) positions and
//! maximum travels are **positive**.

use crate::{
    BOUNDARY, GRID, ORIGIN, TOOL, View,
    config::Point,
    machine::{Arc, CircularDirection, Line, MotionSummary, PlanarPoint},
    parser::Plane,
};
use std::{cmp::Ordering, f32::consts::PI, mem::size_of};
use winit::dpi::PhysicalSize;

/// Stroke width for toolpath [`LineInstance`]s in pixels.
const DEFAULT_STROKE_WIDTH: f32 = 1.25;
/// Stroke width for static [`LineInstance`]s showing the machine boundary box.
const MACHINE_BOUNDARY_WIDTH: f32 = DEFAULT_STROKE_WIDTH * 2.0;
/// Stroke width for static [`LineInstance`]s showing all axes, rooted at origin.
const ORIGIN_WIDTH: f32 = DEFAULT_STROKE_WIDTH * 2.0;
/// Stroke width for static [`LineInstance`]s showing the XY plane grid.
const GRID_WIDTH: f32 = DEFAULT_STROKE_WIDTH * 0.8;

/// Additional padding applied to the machine boundary in pixels.
const MACHINE_INSET: f32 = 10.0;

const MACHINE_BOUNDARY_COLOR: [f32; 3] = [0.69, 0.69, 0.69]; // noice
const RAPID_MOVE_COLOR: [f32; 3] = [1.0, 0.05, 0.05];
const FEED_MOVE_COLOR: [f32; 3] = [0.1, 1.0, 0.1];
const GRID_COLOR: [f32; 3] = [0.1, 0.1, 0.1];
const X_AXIS_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
const Y_AXIS_COLOR: [f32; 3] = [0.0, 1.0, 0.0];
const Z_AXIS_COLOR: [f32; 3] = [0.0, 0.0, 1.0];
const TOOL_COLOR: [f32; 4] = [0.25, 0.25, 0.25, 1.0];

/// Machine units travelled per frame.
const SPEED: f32 = 5.0;

const COS30: f32 = 0.8660254;
const SIN30: f32 = 0.5;

/// Configuration of fixed [`LineInstance`]s that can be toggled.
#[derive(Clone, Copy)]
pub struct StaticConfig {
    /// Whether to render machine travels boundary box.
    machine_boundary: bool,
    /// Whether to render the reference grid on [`Plane::XY`].
    grid: bool,
    /// Whether to render X, Y, and Z axis indicators, rooted at origin.
    origin: bool,
}

impl Default for StaticConfig {
    /// Generates a default config for static [`LineInstances`]s,
    /// based on the compile-time constants.
    fn default() -> Self {
        Self {
            machine_boundary: BOUNDARY,
            grid: GRID,
            origin: ORIGIN,
        }
    }
}

impl StaticConfig {
    /// Sets machine travel boundary box on or off.
    pub fn set_machine_boundary(&mut self, boundary: bool) {
        self.machine_boundary = boundary
    }

    /// Sets the XY plane reference grid on or off.
    pub fn set_grid(&mut self, grid: bool) {
        self.grid = grid
    }

    /// Sets all axis indicators on or off.
    pub fn set_origin(&mut self, origin: bool) {
        self.origin = origin
    }
}

/// Represents a straight line between two points,
/// that can be drawn to the screen with a vertex shader.
///
/// The vertex shader creates 6 vertices (two triangles) per line instance,
/// to create a line with variable thickness.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInstance {
    /// 3D start point of the line.
    start: [f32; 3],
    /// 3D end point of the line.
    end: [f32; 3],
    /// RGB color of the line.
    color: [f32; 3],
    /// Width of the rendered line, in pixels.
    stroke_width: f32,
    /// Depth of the instance in NDC, used inside the shader.
    /// Must be in range `0..1`.
    depth: f32,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }

    pub fn indices() -> [u16; 6] {
        [0, 1, 2, 2, 1, 3] // counter clockwise
    }

    /// Generates a vector of [`LineInstance`]s which render the static geometry (boundary, grid,
    /// orgin), based on the maximum machine travels and [`StaticConfig`] supplied.
    ///
    /// The grid sits the farthest and machine boundary the nearest, with origin lines in between.
    pub fn statics(max_travels: Point, static_config: StaticConfig) -> Vec<Self> {
        let x = max_travels.x;
        let y = max_travels.y;
        let z = max_travels.z;
        let avg = (x + y + z) / 3.0;

        // grid square size, in machine units
        let grid_step = (avg / 10.0).ceil();

        // draw 3 times the axis travel in pos dir and 2 times the axis travel in neg
        let y_lines_count = ((x * 5.0) / grid_step).ceil() as usize;
        let x_lines_count = ((y * 5.0) / grid_step).ceil() as usize;

        // preallocate for 12 boundary edges, 3 axes indicators,
        // y_lines_count vertical grid lines and x_lines_count horizontal grid lines
        let mut ret = Vec::with_capacity(12 + 3 + y_lines_count + x_lines_count);

        let boundary_stroke_width = if static_config.machine_boundary {
            MACHINE_BOUNDARY_WIDTH
        } else {
            0.0
        };
        let grid_stroke_width = if static_config.grid { GRID_WIDTH } else { 0.0 };
        let origin_stroke_width = if static_config.origin {
            ORIGIN_WIDTH
        } else {
            0.0
        };

        // boundary
        ret.extend_from_slice(&[
            Self {
                start: [x, y, z],
                end: [0.0, y, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, y, z],
                end: [x, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [0.0, y, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, 0.0, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, y, 0.0],
                end: [0.0, y, 0.0],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, y, 0.0],
                end: [x, 0.0, 0.0],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [0.0, y, 0.0],
                end: [0.0, 0.0, 0.0],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, 0.0, 0.0],
                end: [0.0, 0.0, 0.0],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, y, 0.0],
                end: [x, y, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [x, 0.0, 0.0],
                end: [x, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [0.0, y, 0.0],
                end: [0.0, y, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
            Self {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: boundary_stroke_width,
                depth: 0.1,
            },
        ]);

        // origin
        ret.extend_from_slice(&[
            Self {
                start: [0.0, 0.0, 0.0],
                end: [x * 5.0, 0.0, 0.0],
                color: X_AXIS_COLOR,
                stroke_width: origin_stroke_width,
                depth: 0.25,
            },
            Self {
                start: [0.0, 0.0, 0.0],
                end: [0.0, y * 5.0, 0.0],
                color: Y_AXIS_COLOR,
                stroke_width: origin_stroke_width,
                depth: 0.25,
            },
            Self {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 0.0, z * 5.0],
                color: Z_AXIS_COLOR,
                stroke_width: origin_stroke_width,
                depth: 0.25,
            },
        ]);

        // grid
        let mut current_x = 0.0;
        let mut current_y = 0.0;

        while current_x < x * 3.0 {
            ret.push(Self {
                start: [current_x, -y * 5.0, 0.0],
                end: [current_x, y * 5.0, 0.0],
                color: GRID_COLOR,
                stroke_width: grid_stroke_width,
                depth: 0.75,
            });
            current_x += grid_step;
        }

        current_x = 0.0;

        while current_x > -x * 2.0 {
            ret.push(Self {
                start: [current_x, -y * 5.0, 0.0],
                end: [current_x, y * 5.0, 0.0],
                color: GRID_COLOR,
                stroke_width: grid_stroke_width,
                depth: 0.75,
            });
            current_x -= grid_step;
        }

        while current_y < y * 3.0 {
            ret.push(Self {
                start: [-x * 5.0, current_y, 0.0],
                end: [x * 5.0, current_y, 0.0],
                color: GRID_COLOR,
                stroke_width: grid_stroke_width,
                depth: 0.75,
            });
            current_y += grid_step;
        }

        current_y = 0.0;

        while current_y > -y * 2.0 {
            ret.push(Self {
                start: [-x * 5.0, current_y, 0.0],
                end: [x * 5.0, current_y, 0.0],
                color: GRID_COLOR,
                stroke_width: grid_stroke_width,
                depth: 0.75,
            });
            current_y -= grid_step;
        }

        ret
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// with [`DEFAULT_STROKE_WIDTH`] and [`RAPID_MOVE_COLOR`].
    pub fn rapid_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            color: RAPID_MOVE_COLOR,
            stroke_width: DEFAULT_STROKE_WIDTH,
            depth: 0.5,
        }
    }

    /// Creates a single [`LineInstance`] from `start` to `end`,
    /// with [`DEFAULT_STROKE_WIDTH`] and [`FEED_MOVE_COLOR`].
    pub fn feed_move(start: Point, end: Point) -> Self {
        Self {
            start: [start.x, start.y, start.z],
            end: [end.x, end.y, end.z],
            color: FEED_MOVE_COLOR,
            stroke_width: DEFAULT_STROKE_WIDTH,
            depth: 0.5,
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
    pub fn next(&mut self) -> BufferAction {
        let instances = self
            .instances
            .as_mut()
            .expect("Next instance requested without before adding new instances.");

        let res = match instances {
            LineInstances::Linear(lines) => lines.next(),
            LineInstances::Arc(lines) => lines.next(),
        };

        let instance = match res {
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

        let ret = match instances {
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

/// Represents the current 3D position of the tool,
/// that can be drawn to the screen with a vertex shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ToolInstance {
    position: [f32; 3],
}

impl ToolInstance {
    /// Returns a [`VertexBufferLayout`](wgpu::VertexBufferLayout) that describes how the
    /// [`ToolInstance`] is stored in a GPU buffer.
    ///
    /// The layout is set to use [`VertexStepMode::Instance`](wgpu::VertexStepMode::Instance),
    /// which allows the vertex shader to expand a single 3D point to a cylinderical tool,
    /// with its bottom center at the tool position.
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }

    /// Creates a new [`ToolInstance`], which will be rendered at the provided [`Point`].
    pub fn at_point(point: Point) -> Self {
        Self {
            position: [point.x, point.y, point.z],
        }
    }

    /// Creates a new [`ToolInstance`], which will be rendered at [`LineInstance::end`].
    pub fn at_line_end(instance: LineInstance) -> Self {
        Self {
            position: instance.end,
        }
    }
}

/// Represents the constant data to be shared across
/// all [`LineInstance`]s and the [`ToolInstance`], per frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// Width and height of the surface.
    window_size: [f32; 2],
    /// Padding for alignment.
    _pad1: [f32; 2],
    /// Maximum axis travels for each axis of the machine.
    /// The first three number correspond to X, Y, and Z axis travels respectively.
    /// The last value is used for alignment and is never used.
    max_travels: [f32; 4],
    /// View matrix to center the machine volume and scale it to [`Self::window_size`].
    /// Multiplication with this matrix results in **pixel** units.
    projection: [[f32; 4]; 4],
    /// Color for rendering the [`ToolInstance`].
    tool_color: [f32; 4],
    /// Diameter of the tool to render.
    tool_size: f32,
    /// Length of the tool to render.
    tool_len: f32,
    /// Active [`View`].
    view: View,
    _pad2: f32,
}

impl Uniforms {
    /// Constructs a new [`Uniforms`] with `view` set to [`View::default`].
    pub fn new(window_size: PhysicalSize<u32>, max_travels: Point) -> Self {
        let window_size = [window_size.width as f32, window_size.height as f32];
        let max_travels = [max_travels.x, max_travels.y, max_travels.z, 0.0];
        let view = View::default();

        let machine_size = machine_size(max_travels.as_slice(), view);
        let scale = scale(window_size, machine_size);
        let offset = offset(max_travels, machine_size, scale, view);

        Self {
            window_size,
            _pad1: [0.0, 0.0],
            projection: projection_matrix(view, scale, offset),
            max_travels,
            tool_color: TOOL_COLOR,
            tool_size: if TOOL {
                max_travels[0].abs() / 40.0
            } else {
                0.0
            },
            tool_len: max_travels[2].abs() / 2.0,
            view,
            _pad2: 0.0,
        }
    }

    /// Recalculates [`Self::projection`] view matrix for a new `window_size`.
    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = [window_size.width as f32, window_size.height as f32];
        let machine_size = machine_size(self.max_travels.as_slice(), self.view);
        let scale = scale(self.window_size, machine_size);
        let offset = offset(self.max_travels, machine_size, scale, self.view);
        self.projection = projection_matrix(self.view, scale, offset);
    }

    /// Returns the active [`View`].
    pub fn view(&self) -> View {
        self.view
    }

    /// Changes the active view and recalculates [`Self::projection`].
    pub fn set_view(&mut self, view: View) {
        self.view = view;
        self.resize(PhysicalSize {
            width: self.window_size[0] as u32,
            height: self.window_size[1] as u32,
        });
    }

    /// Sets tool visibility by:
    /// - Setting [`Self::tool_size`] to `0.0` to hide.
    /// - Recalculating [`Self::tool_size`] from [`Self::max_travels`] to show.
    pub fn set_tool(&mut self, tool: bool) {
        self.tool_size = if tool {
            (self.max_travels[0].abs() + self.max_travels[1].abs() + self.max_travels[2].abs())
                / 100.0
        } else {
            0.0
        }
    }
}

/// Returns the size of a rectangle that would be needed to fit a machine with `max_travels`,
/// rendered from the provided [`View`].
///
/// Does not account for **machine boundary width**.
///
/// The returned size will be in the same units as `max_travels`.
fn machine_size(max_travels: &[f32], view: View) -> [f32; 2] {
    match view {
        // use projection of the bounding box to get final x and y
        View::Isometric => project_bounding_box(max_travels),
        // use x and y of the machine
        View::Top => [max_travels[0], max_travels[1]],
    }
}

/// Returns the size of a rectangle that would be needed to fit a machine with `max_travels`,
/// rendered from [`View::Isometric`].
///
/// The returned size will be in the same units as `max_travels`.
fn project_bounding_box(max_travels: &[f32]) -> [f32; 2] {
    [
        (max_travels[0] + max_travels[1]) * COS30,
        (max_travels[0] + max_travels[1]) * SIN30 + max_travels[2],
    ]
}

/// Computes the scaling factor, in **pixels per machine unit**
/// that fits the machine inside the window,
/// accounting for half of machine boundary extending over `machine_size` on each side and
/// [`MACHINE_INSET`].
///
/// The provided `machine_size` must be the size **AFTER** any projection.
///
/// The returned scale will prioritize fitting the axis that is longer relative to that of the window.
fn scale(window_size: [f32; 2], machine_size: [f32; 2]) -> f32 {
    // y / x
    // compensate for half of the boundary width in window, per side, and apply any inset
    let usable_width = window_size[0] - MACHINE_BOUNDARY_WIDTH - MACHINE_INSET;
    let usable_height = window_size[1] - MACHINE_BOUNDARY_WIDTH - MACHINE_INSET;

    let window_ratio = usable_height / usable_width;
    let machine_ratio = machine_size[1] / machine_size[0];

    match machine_ratio.total_cmp(&window_ratio) {
        // y of machine is smaller, scale to fit x of machine and shrink in y
        Ordering::Less => usable_width / machine_size[0],
        // choose any
        Ordering::Equal => usable_width / machine_size[0],
        // y of machine is larger, scale to fit y of machine and shrink in x
        Ordering::Greater => usable_height / machine_size[1],
    }
}

/// Computes the offset, in **pixels** that centers the machine inside the window, for a [`View`].
///
/// The provided `machine_size` must be the size **AFTER** any projection.
fn offset(max_travels: [f32; 4], machine_size: [f32; 2], scale: f32, view: View) -> [f32; 2] {
    match view {
        View::Isometric => [
            // half of machine size works because 0 of machine will at the boundary
            -(machine_size[0] * scale) / 2.0,
            // half of machine size does not work because the y 0 of the machine view is not at the boundary
            ((max_travels[0] - max_travels[1]) * SIN30 - max_travels[2]) * scale / 2.0,
        ],
        View::Top => [
            -(machine_size[0] * scale) / 2.0,
            -(machine_size[1] * scale) / 2.0,
        ],
    }
}

/// Constructs a view-projection matrix for a provided [`View`],
/// scales the vertices & center the view volume using provided `offset`.
fn projection_matrix(view: View, scale: f32, offset: [f32; 2]) -> [[f32; 4]; 4] {
    // the actual matrix would visually be the transpose of the return value, row first
    match view {
        View::Isometric => [
            [scale * COS30, -scale * SIN30, 0.0, 0.0],
            [scale * COS30, scale * SIN30, 0.0, 0.0],
            [0.0, scale, 0.0, 0.0],
            [offset[0], offset[1], 0.0, 1.0],
        ],
        View::Top => [
            [scale, 0.0, 0.0, 0.0],
            [0.0, scale, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [offset[0], offset[1], 0.0, 1.0],
        ],
    }
}
