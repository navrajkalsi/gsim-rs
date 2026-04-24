use std::cmp::Ordering;

use winit::dpi::PhysicalSize;

use crate::parser::Point;

const DEFAULT_STROKE_WIDTH: f32 = 0.0025;
const MACHINE_BOUNDARY_WIDTH: f32 = DEFAULT_STROKE_WIDTH * 5.0;
const MACHINE_BOUNDARY_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
const RAPID_MOVE_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
const FEED_MOVE_COLOR: [f32; 3] = [0.0, 1.0, 0.0];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    start: [f32; 3],
    end: [f32; 3],
    color: [f32; 3],
    stroke_width: f32,
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
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }

    pub fn machine_boundary(max_travels: &Point) -> [Self; 4] {
        let x = max_travels.x() as f32;
        let y = max_travels.y() as f32;
        let z = max_travels.z() as f32;

        [
            Self {
                start: [x, y, z],
                end: [0.0, y, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: MACHINE_BOUNDARY_WIDTH,
            },
            Self {
                start: [x, y, z],
                end: [x, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: MACHINE_BOUNDARY_WIDTH,
            },
            Self {
                start: [0.0, y, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: MACHINE_BOUNDARY_WIDTH,
            },
            Self {
                start: [x, 0.0, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
                stroke_width: MACHINE_BOUNDARY_WIDTH,
            },
        ]
    }

    pub fn rapid_move(start: &Point, end: &Point) -> Self {
        Self {
            start: [start.x() as f32, start.y() as f32, start.z() as f32],
            end: [end.x() as f32, end.y() as f32, end.z() as f32],
            color: RAPID_MOVE_COLOR,
            stroke_width: DEFAULT_STROKE_WIDTH,
        }
    }

    pub fn feed_move(start: &Point, end: &Point) -> Self {
        Self {
            start: [start.x() as f32, start.y() as f32, start.z() as f32],
            end: [end.x() as f32, end.y() as f32, end.z() as f32],
            color: FEED_MOVE_COLOR,
            stroke_width: DEFAULT_STROKE_WIDTH,
        }
    }
}

// multiply this to machine units to get the number of pixels
fn get_scale(window_size: [f32; 2], max_travels: [f32; 2]) -> f32 {
    let machine_size = [max_travels[0].abs(), max_travels[1].abs()];
    // y / x
    let window_ratio = window_size[1] / window_size[0];
    let machine_ratio = machine_size[1] / machine_size[0];

    let scale = match machine_ratio.total_cmp(&window_ratio) {
        // y of machine is smaller, scale to fit x of machine and shrink in y
        Ordering::Less => window_size[0] / machine_size[0],
        // choose any
        Ordering::Equal => window_size[0] / machine_size[0],
        // y of machine is larger, scale to fit y of machine and shrink in x
        Ordering::Greater => window_size[1] / machine_size[1],
    };

    // reduce scale to compensate for machine boundary thickness on both sides
    scale - MACHINE_BOUNDARY_WIDTH
}

fn get_padding(window_size: [f32; 2], max_travels: [f32; 2], scale: f32) -> [f32; 2] {
    [
        // compensate for machine boundary width
        (window_size[0] - max_travels[0].abs() * scale) / 2.0,
        (window_size[1] - max_travels[1].abs() * scale) / 2.0,
    ]
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    window_size: [f32; 2],
    // padding in pixels to center machine view inside the window
    padding: [f32; 2],
    // signed max travels for each axis, starting at 0 for each axis
    max_travels: [f32; 4],
    // absolute scale, to convert machine unit to pixels
    scale: f32,
    _pad: [f32; 3],
}

impl Uniforms {
    pub fn new(window_size: PhysicalSize<u32>, max_travels: &Point) -> Self {
        let window_size = [window_size.width as f32, window_size.height as f32];
        let max_travels = [
            max_travels.x() as f32,
            max_travels.y() as f32,
            max_travels.z() as f32,
            0.0,
        ];
        let scale = get_scale(window_size, [max_travels[0], max_travels[1]]);

        Self {
            window_size,
            padding: get_padding(window_size, [max_travels[0], max_travels[1]], scale),
            max_travels,
            scale,
            _pad: [0.0, 0.0, 0.0],
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.window_size = [new_size.width as f32, new_size.height as f32];
        self.scale = get_scale(self.window_size, [self.max_travels[0], self.max_travels[1]]);
        self.padding = get_padding(
            self.window_size,
            [self.max_travels[0], self.max_travels[1]],
            self.scale,
        );
    }
}
