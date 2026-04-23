use std::cmp::Ordering;

use winit::dpi::PhysicalSize;

use crate::parser::Point;

const DEFAULT_STROKE_WIDTH: f32 = 0.01;
const MACHINE_BOUNDARY_COLOR: [f32; 3] = [0.5, 0.5, 0.5];

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

    pub fn machine_boundary(max_travels: &Point) -> [Self; 4] {
        let x = max_travels.x() as f32;
        let y = max_travels.y() as f32;
        let z = max_travels.z() as f32;

        [
            Self {
                start: [x, y, z],
                end: [0.0, y, z],
                color: MACHINE_BOUNDARY_COLOR,
            },
            Self {
                start: [x, y, z],
                end: [x, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
            },
            Self {
                start: [0.0, y, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
            },
            Self {
                start: [x, 0.0, z],
                end: [0.0, 0.0, z],
                color: MACHINE_BOUNDARY_COLOR,
            },
        ]
    }
}

// multiply this to machine units to get the number of pixels
fn get_scale(window_size: [f32; 2], max_travels: [f32; 2]) -> f32 {
    let machine_size = [max_travels[0].abs(), max_travels[1].abs()];
    // y / x
    let window_ratio = window_size[1] / window_size[0];
    let machine_ratio = machine_size[1] / machine_size[0];

    match machine_ratio.total_cmp(&window_ratio) {
        // y of machine is smaller, scale to fit x of machine and shrink in y
        Ordering::Less => window_size[0] / machine_size[0],
        // choose any
        Ordering::Equal => window_size[0] / machine_size[0],
        // y of machine is larger, scale to fit y of machine and shrink in x
        Ordering::Greater => window_size[1] / machine_size[1],
    }
}

fn get_padding(window_size: [f32; 2], max_travels: [f32; 2], scale: f32) -> [f32; 2] {
    [
        (window_size[0] - max_travels[0] * scale) / 2.0,
        (window_size[1] - max_travels[1] * scale) / 2.0,
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
    stroke_width: f32,
    _pad: [f32; 2],
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
            max_travels,
            scale,
            padding: get_padding(window_size, [max_travels[0], max_travels[1]], scale),
            stroke_width: DEFAULT_STROKE_WIDTH,
            _pad: [0.0, 0.0],
        }
    }
}

pub fn clip_machine_pos(
    pos: (f32, f32),
    window_size: (f32, f32),
    machine_size: (f32, f32),
) -> (f32, f32) {
    let scaling_factor = get_scale(
        [window_size.0, window_size.1],
        [machine_size.0, machine_size.1],
    );

    // number of pixels from the machine zero corner of screen
    // (this corner may or may not be the 0 points of the window)
    let scaled_machine_pos = (pos.0 * scaling_factor, pos.1 * scaling_factor);
    let scaled_machine_size = (
        machine_size.0.abs() * scaling_factor,
        machine_size.1.abs() * scaling_factor,
    );
    // for centering the machine view
    let padding = (
        (window_size.0 - scaled_machine_size.0) / 2.0,
        (window_size.1 - scaled_machine_size.1) / 2.0,
    );

    // position on screen with respect to the window coordinate system(0 on top left corner)
    // without padding
    let window_pos = match (
        machine_size.0.is_sign_positive(),
        machine_size.1.is_sign_positive(),
    ) {
        // machine zero on lower left corner, all positive vals
        (true, true) => (scaled_machine_pos.0, window_size.1 - scaled_machine_pos.1),
        // machine zero on top left corner, negative y vals
        (true, false) => (scaled_machine_pos.0, scaled_machine_pos.1.abs()),
        // machine zero on lower right corner, negative x vals
        (false, true) => (
            window_size.0 - scaled_machine_pos.0.abs(),
            window_size.1 - scaled_machine_pos.1,
        ),
        // machine zero on top right corner, all negative vals
        (false, false) => (
            window_size.0 - scaled_machine_pos.0.abs(),
            scaled_machine_pos.1.abs(),
        ),
    };

    // add padding
    let window_pos = (window_pos.0 + padding.0, window_pos.1 + padding.1);

    // flip y to match coordinate system of clip space
    (
        (window_pos.0 / window_size.0) * 2.0 - 1.0,
        1.0 - (window_pos.1 / window_size.1) * 2.0,
    )
}

// think of a way to supply maxtravels on ap startup
