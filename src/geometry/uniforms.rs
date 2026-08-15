use crate::{
    geometry::{math::Matrix, view::View},
    points::Point,
};
use winit::dpi::PhysicalSize;

/// Defines the maximum amount the default scaling factor can change in relation to itself,
/// at runtime due to user input.
const MAX_SCALE_MANIPULATION: f32 = 0.75;

/// Custom factor for converting number of lines scrolled to the simulation scaling factor.
const LINES_TO_SCALE_FACTOR: f32 = 0.1;

const MOUSE_SENSITIVITY: f32 = 0.005;

/// Represents the constant data to be shared across all geometric instances, per frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms(Matrix);

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

// cube edge in machine units
fn max_bounding_cube_edge(stock_size: [f32; 3]) -> f32 {
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

#[derive(Copy, Clone, Debug)]
pub struct Transform {
    stock_size: [f32; 3],
    // stores current rotations,
    // also centers in machine units
    orientation: Matrix,
    scales: [f32; 3],
    translations: [f32; 3],
    window_size: [f32; 2],
    bounding_cube_edge: f32,
    user_scale: f32,
    view: Option<View>,
}

impl Transform {
    /// Constructs a new [`Uniforms`] with `view` set to [`View::default`].
    pub fn new(window_size: PhysicalSize<u32>, stock_size: Point) -> Self {
        let stock_size = stock_size.as_array();
        let window_size = [window_size.width as f32, window_size.height as f32];
        let bounding_cube_edge = max_bounding_cube_edge(stock_size);
        // cannot scale directly to ndc as the volume is not a square
        // we NEED to go through pixels
        let xy_scale = xy_scale(window_size, bounding_cube_edge); // to pixels
        let z_scale = z_scale(bounding_cube_edge); // direct ndc

        Self {
            stock_size,
            orientation: Matrix::new(stock_size).rotate(View::default().rotations()),
            scales: [xy_scale, xy_scale, z_scale],
            translations: [0.0, 0.0, 0.5], // lifts up z to the center
            window_size,
            bounding_cube_edge,
            user_scale: 0.0,
            view: Some(View::default()),
        }
    }

    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = [window_size.width as f32, window_size.height as f32];
        self.scales[0] = xy_scale(self.window_size, self.bounding_cube_edge); // to pixels
        self.scales[1] = self.scales[0];
    }

    /// Returns the active [`View`].
    pub fn view(&self) -> Option<View> {
        self.view
    }

    /// Changes the active view and recalculates [`Self::projection`].
    pub fn set_view(&mut self, view: View) {
        self.orientation = Matrix::new(self.stock_size).rotate(view.rotations());
        self.view = Some(view);
    }

    pub fn translate(&mut self, delta: [f32; 2]) {
        self.translations[0] += delta[0];
        self.translations[1] += delta[1];
    }

    pub fn scale(&mut self, lines_scrolled: f32) {
        self.user_scale += lines_scrolled * LINES_TO_SCALE_FACTOR;

        const {
            debug_assert!(MAX_SCALE_MANIPULATION >= 0.1 || MAX_SCALE_MANIPULATION <= 0.9);
        }

        let max_scale_manipulation = self.scales[0] * MAX_SCALE_MANIPULATION; // x and y scales are
        // same

        // half lower limit, double upper limit
        // therefore more relative zoom in is possible than zoom out
        self.user_scale = self
            .user_scale
            .clamp(-max_scale_manipulation / 2.0, max_scale_manipulation * 2.0);
    }

    pub fn rotate(&mut self, delta: [f32; 3]) {
        let rotations = [
            delta[1] * MOUSE_SENSITIVITY, // mouse movement in y rotates around x
            delta[0] * MOUSE_SENSITIVITY,
            delta[2] * MOUSE_SENSITIVITY,
        ];

        self.orientation = self.orientation.rotate(rotations);

        self.view = None;
    }

    pub fn reset_translations(&mut self) {
        self.translations[0] = 0.0;
        self.translations[1] = 0.0;
        // do not reset z translations as we need to pull z up in the center range
    }

    pub fn reset_scale(&mut self) {
        self.user_scale = 0.0
    }

    // adds any user scales
    fn final_scales(&self) -> [f32; 3] {
        [
            self.scales[0] + self.user_scale,
            self.scales[1] + self.user_scale,
            self.scales[2],
        ]
    }
}

impl From<Transform> for Uniforms {
    fn from(transform: Transform) -> Self {
        Uniforms(
            transform
                .orientation
                .scale(transform.final_scales()) // xy in pixels, z in ndc
                .translate(transform.translations) // add panning in pixels and lift up z in ndc
                .scale([
                    2.0 / transform.window_size[0],
                    2.0 / transform.window_size[1],
                    1.0,
                ]), // convert to ndc
        )
    }
}
