use std::f32::consts::PI;

use crate::{View, points::Point};
use winit::dpi::PhysicalSize;

/// Defines the maximum amount the default scaling factor can change in relation to itself,
/// at runtime due to user input.
const MAX_SCALE_MANIPULATION: f32 = 0.75;

/// Custom factor for converting number of lines scrolled to the simulation scaling factor.
const LINES_TO_SCALE_FACTOR: f32 = 0.25;

const COS30: f32 = 0.8660254;
const SIN30: f32 = 0.5;
const COS45: f32 = 0.707107;

/// Represents the constant data to be shared across all geometric instances, per frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    center: [[f32; 4]; 4], // translation
    scale: [[f32; 4]; 4],
    //rotation is done before scaling so that final z is scaled to 0 and 1
    //and xy can be scaled to -1 to 1
    x_rotation: [[f32; 4]; 4],
    y_rotation: [[f32; 4]; 4],
    z_rotation: [[f32; 4]; 4],
    stock_size: [f32; 4],
    window_size: [f32; 2],
    bounding_cube_edge: f32,
    user_scale: f32,
}

impl Uniforms {
    /// Constructs a new [`Uniforms`] with `view` set to [`View::default`].
    pub fn new(window_size: PhysicalSize<u32>, stock_size: Point) -> Self {
        let window_size = [window_size.width as f32, window_size.height as f32];
        let stock_size = [stock_size.x, stock_size.y, stock_size.z, 0.0];

        let center = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                -stock_size[0] / 2.0,
                -stock_size[1] / 2.0,
                -stock_size[2] / 2.0,
                1.0,
            ],
        ];

        let bounding_cube_edge = max_bounding_cube_edge(stock_size.as_slice());

        // cannot scale directly to ndc as the volume is not a square
        // we NEED to go through pixels
        let xy_scale = xy_scale(window_size, bounding_cube_edge) * 2.0; // to pixels, for offsetting
        let z_scale = z_scale(bounding_cube_edge); // direct ndc

        let scale = [
            [xy_scale, 0.0, 0.0, 0.0],
            [0.0, xy_scale, 0.0, 0.0],
            [0.0, 0.0, z_scale, 0.0],
            [0.0, 0.0, 0.5, 1.0],
        ];

        Self {
            center,
            scale,
            x_rotation: x_rotation(0.0),
            y_rotation: y_rotation(0.0),
            z_rotation: z_rotation(0.0),
            stock_size,
            window_size,
            bounding_cube_edge,
            user_scale: 0.0,
        }
    }

    /// Recalculates [`Self::projection`] view matrix for a new `window_size`.
    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = [window_size.width as f32, window_size.height as f32];

        let xy_scale = xy_scale(self.window_size, self.bounding_cube_edge) * 2.0; // to pixels, for offsetting

        debug_assert!(MAX_SCALE_MANIPULATION >= 0.1 || MAX_SCALE_MANIPULATION <= 0.9);
        let max_scale_manipulation = xy_scale * MAX_SCALE_MANIPULATION;

        // half rel min limit, double rel max limit
        self.user_scale = self
            .user_scale
            .clamp(-max_scale_manipulation / 2.0, max_scale_manipulation * 2.0);

        self.scale[0][0] = xy_scale + self.user_scale;
        self.scale[1][1] = xy_scale + self.user_scale;
    }

    /// Returns the active [`View`].
    pub fn view(&self) -> View {
        // self.view
        View::Isometric
    }

    /// Changes the active view and recalculates [`Self::projection`].
    pub fn set_view(&mut self, view: View) {
        let (x, y, z) = match view {
            View::Isometric => ((PI / 2.0) - 0.615472907, 0.0, -PI / 4.0),
            View::Top => (0.0, 0.0, 0.0),
        };

        self.x_rotation = x_rotation(x);
        self.y_rotation = y_rotation(y);
        self.z_rotation = z_rotation(z);
    }

    pub fn add_user_scale(&mut self, lines_scrolled: f32) {
        self.user_scale += lines_scrolled * LINES_TO_SCALE_FACTOR;

        self.resize(PhysicalSize {
            width: self.window_size[0] as u32,
            height: self.window_size[1] as u32,
        });
    }

    pub fn add_user_offset(&mut self, delta: [f32; 2]) {
        self.scale[3][0] += delta[0] * 2.0; // double since scale is being also being doubled
        self.scale[3][1] += delta[1] * 2.0;
    }

    pub fn add_user_projection(&mut self, delta: [f32; 2]) {
        // // arbitrary distance of view window from stock center
        // let window_dist = self.stock_size[0] + self.stock_size[1] + self.stock_size[2];
        //
        // let x_angle = (delta[0] / 2.0 / window_dist).clamp(-1.0, 1.0).asin() * 2.0;
        // let y_angle = (delta[1] / 2.0 / window_dist).clamp(-1.0, 1.0).asin() * 2.0;
        //
        // eprintln!("x: {x_angle}");
        // eprintln!("y: {y_angle}");
        // eprintln!("");

        let x_angle = delta[1] / 100.0;
        let y_angle = delta[0] / 100.0;
    }
}

/// Returns the size of a rectangular view that would be needed to fit a stock with `stock_size`,
/// rendered from the provided [`View`].
///
/// The returned size will be in the same units as `stock_size`.
///
/// This is only used for predefined views.
fn stock_view(stock_size: &[f32], view: View) -> [f32; 2] {
    match view {
        // use projection of the bounding box to get final x and y
        View::Isometric => isometric_stock_view(stock_size),
        // use x and y of the stock
        View::Top => [stock_size[0], stock_size[1]],
    }
}

/// Returns the size of a rectangular view that would be needed to fit a stock with `stock_size`,
/// rendered from [`View::Isometric`].
///
/// The returned size will be in the same units as `stock_size`.
fn isometric_stock_view(stock_size: &[f32]) -> [f32; 2] {
    [
        (stock_size[0] + stock_size[1]) * COS30,
        (stock_size[0] + stock_size[1]) * SIN30 + stock_size[2],
    ]
}

/// Computes the scaling factor, in **pixels per stock unit**
/// that fits the stock inside the window, accounting for [`STOCK_INSET`].
///
/// The provided `stock_view` must be the size **AFTER** any projection.
///
/// The returned scale will prioritize fitting the dimension that is longer relative to that of the window.
// only xy scale
// since we max bounding shape is a cube, we need to scale according to the
// window dimension that is smaller
fn xy_scale(window_size: [f32; 2], bounding_cube_edge: f32) -> f32 {
    // scale to fit in x
    if window_size[0] < window_size[1] {
        window_size[0] / bounding_cube_edge
    } else {
        window_size[1] / bounding_cube_edge
    }
}

/// Computes the offset, in **pixels** that centers the stock inside the window, for a [`View`].
///
/// The provided `stock_view` must be the size **AFTER** any projection.
fn offset(stock_size: [f32; 4], stock_view: [f32; 2], scale: f32, view: View) -> [f32; 2] {
    match view {
        View::Isometric => [
            // half of stock view works because 0 of the stock will be at an edge
            -(stock_view[0] * scale) / 2.0,
            // half of stock_view does not work because the y 0 of the stock view is not at an edge
            ((stock_size[0] - stock_size[1]) * SIN30 - stock_size[2]) * scale / 2.0,
        ],
        View::Top => [
            -(stock_size[0] * scale) / 2.0,
            -(stock_size[1] * scale) / 2.0,
        ],
    }
}

/// Constructs a view-projection matrix for a provided [`View`],
/// scales the vertices & center the view volume using provided `offset`.
fn projection_matrix(
    view: View,
    scale: f32,
    offset: [f32; 2],
    stock_size: &[f32],
    x_angle: f32,
) -> [[f32; 4]; 4] {
    // the actual matrix would visually be the transpose of the return value, row first
    let x = stock_size[0];
    let y = stock_size[1];
    let z = stock_size[2];

    let y_angle: f32 = 0.0;

    let ratio = (y / x) * 0.5; // target to put all the stock boundary in middle 0.5 depth
    match view {
        View::Isometric => [
            [scale * COS30, -scale * SIN30, -ratio / x, 0.0],
            [scale * COS30, scale * SIN30, ratio / y, 0.0],
            [0.0, scale, 0.0, 0.0],
            [offset[0], offset[1], ratio, 1.0],
        ],
        View::Top => [
            [scale, 0.0, 0.0, 0.0],
            [0.0, scale, 0.0, 0.0],
            [0.0, 0.0, -0.5 / z, 0.0],
            [offset[0], offset[1], 0.75, 1.0],
        ],
    }
}

// cube edge in machine units
fn max_bounding_cube_edge(stock_size: &[f32]) -> f32 {
    // dont take root
    let xy_diagonal_sqr = stock_size[0].powi(2) + stock_size[1].powi(2);

    (xy_diagonal_sqr + stock_size[2].powi(2)).sqrt()
}

// returns factor to fit the supplied cube edge into 0.5
// add 0.5 to pull z up into the view has the volume
//
// we are scaling to 0.5 to have extra padding in depth for tool
// since without adding any value our z will be in half neg and half pos,ie, centered around 0.0.
// we need to center it around 0.5 therefore we need to add 0.5
fn z_scale(bounding_cube_edge: f32) -> f32 {
    0.5 / bounding_cube_edge
}

fn rotation(x_angle: f32, y_angle: f32) -> [[f32; 4]; 4] {
    let x = x_rotation(x_angle);
    let y = y_rotation(y_angle);

    let ret = multiply(x, y);

    ret
}

fn x_rotation(x_angle: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, x_angle.cos(), -x_angle.sin(), 0.0],
        [0.0, x_angle.sin(), x_angle.cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn y_rotation(y_angle: f32) -> [[f32; 4]; 4] {
    [
        [y_angle.cos(), 0.0, -y_angle.sin(), 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [y_angle.sin(), 0.0, y_angle.cos(), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn z_rotation(z_angle: f32) -> [[f32; 4]; 4] {
    [
        [z_angle.cos(), z_angle.sin(), 0.0, 0.0],
        [-z_angle.sin(), z_angle.cos(), 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn multiply(first: [[f32; 4]; 4], second: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    [
        [
            first[0][0] * second[0][0]
                + first[0][1] * second[1][0]
                + first[0][2] * second[2][0]
                + first[0][3] * second[3][0],
            first[0][0] * second[0][1]
                + first[0][1] * second[1][1]
                + first[0][2] * second[2][1]
                + first[0][3] * second[3][1],
            first[0][0] * second[0][2]
                + first[0][1] * second[1][2]
                + first[0][2] * second[2][2]
                + first[0][3] * second[3][2],
            first[0][0] * second[0][3]
                + first[0][1] * second[1][3]
                + first[0][2] * second[2][3]
                + first[0][3] * second[3][3],
        ],
        [
            first[1][0] * second[0][0]
                + first[1][1] * second[1][0]
                + first[1][2] * second[2][0]
                + first[1][3] * second[3][0],
            first[1][0] * second[0][1]
                + first[1][1] * second[1][1]
                + first[1][2] * second[2][1]
                + first[1][3] * second[3][1],
            first[1][0] * second[0][2]
                + first[1][1] * second[1][2]
                + first[1][2] * second[2][2]
                + first[1][3] * second[3][2],
            first[1][0] * second[0][3]
                + first[1][1] * second[1][3]
                + first[1][2] * second[2][3]
                + first[1][3] * second[3][3],
        ],
        [
            first[2][0] * second[0][0]
                + first[2][1] * second[1][0]
                + first[2][2] * second[2][0]
                + first[2][3] * second[3][0],
            first[2][0] * second[0][1]
                + first[2][1] * second[1][1]
                + first[2][2] * second[2][1]
                + first[2][3] * second[3][1],
            first[2][0] * second[0][2]
                + first[2][1] * second[1][2]
                + first[2][2] * second[2][2]
                + first[2][3] * second[3][2],
            first[2][0] * second[0][3]
                + first[2][1] * second[1][3]
                + first[2][2] * second[2][3]
                + first[2][3] * second[3][3],
        ],
        [
            first[3][0] * second[0][0]
                + first[3][1] * second[1][0]
                + first[3][2] * second[2][0]
                + first[3][3] * second[3][0],
            first[3][0] * second[0][1]
                + first[3][1] * second[1][1]
                + first[3][2] * second[2][1]
                + first[3][3] * second[3][1],
            first[3][0] * second[0][2]
                + first[3][1] * second[1][2]
                + first[3][2] * second[2][2]
                + first[3][3] * second[3][2],
            first[3][0] * second[0][3]
                + first[3][1] * second[1][3]
                + first[3][2] * second[2][3]
                + first[3][3] * second[3][3],
        ],
    ]
}
