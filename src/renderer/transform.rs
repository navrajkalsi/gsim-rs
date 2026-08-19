//! # Transform
//!
//! Applies 3D transformations to the simulation. These consist of:
//! - Translating/Panning.
//! - Scaling/Zooming.
//! - Rotating/Orbiting.
//!
//! [`Transform`] keeps track of all these changes, which can be accumulated into an [`Uniforms`].
//!
//! ## Coordinate System
//! All the transformations are done assuming **right-handed** coordinate system.

use crate::{
    geometry::{math::Matrix, uniforms::Uniforms, view::View},
    points::Point,
};
use winit::dpi::PhysicalSize;

/// Defines the maximum amount the default scaling factor can change in relation to itself,
/// at runtime due to user input.
const MAX_SCALE_MANIPULATION: f32 = 0.75;

/// Converts number of lines scrolled to the simulation scaling factor.
const ZOOM_SENSITIVITY: f32 = 0.1;

/// Converts mouse movement (in pixels) to angle (in radians).
const ORBIT_SENSITIVITY: f32 = 0.005;

/// Collection of individual transformations.
#[derive(Copy, Clone, Debug)]
pub struct Transform {
    /// Stock size in machine units.
    stock_size: [f32; 3],

    /// Stores the current rotations, applied to a centered [`Self::stock_size`] matrix.
    orientation: Matrix,

    /// Scaling factors for each axis.
    scales: [f32; 3],

    /// Translations for each axis.
    translations: [f32; 3],

    /// Window size to scale to.
    window_size: [f32; 2],

    /// Edge of the cube that can contain any orientation of [`Self::stock_size`].
    bounding_cube_edge: f32,

    /// Additional scaling factor to be applied in X & Y due to user input.
    user_scale: f32,

    /// Current selected view, that provides its [`default rotations`](View::rotations).
    view: Option<View>,
}

impl Transform {
    /// Constructs a new [`Transform`] for a specific `window_size` and `stock_size`.
    ///
    /// Configures it to use [`View::default`] and its default [`rotations`](View::rotations).
    pub fn new(window_size: PhysicalSize<u32>, stock_size: Point) -> Self {
        let stock_size = stock_size.as_array();
        let window_size = [window_size.width as f32, window_size.height as f32];
        let bounding_cube_edge = max_bounding_cube_edge(stock_size);

        // cannot scale directly to ndc as the volume is not a square
        // we NEED to go through pixels, and we can add translations in pixels
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

    /// Reconfigures [`self`] to use a new `window_size` by calculating new scaling factors
    /// for X & Y axis.
    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = [window_size.width as f32, window_size.height as f32];
        self.scales[0] = xy_scale(self.window_size, self.bounding_cube_edge); // to pixels
        self.scales[1] = self.scales[0];
    }

    /// Returns the active [`View`].
    pub fn view(&self) -> Option<View> {
        self.view
    }

    /// Changes the active view and recalculates current orientation.
    pub fn set_view(&mut self, view: View) {
        self.orientation = Matrix::new(self.stock_size).rotate(view.rotations());
        self.view = Some(view);
    }

    /// Applies translation in pixels along X & Y axes.
    pub fn translate(&mut self, delta: [f32; 2]) {
        self.translations[0] += delta[0];
        self.translations[1] += delta[1];
    }

    /// Applies additional scale to X & Y axis, based on the default XY scale.
    pub fn scale(&mut self, lines_scrolled: f32) {
        self.user_scale += lines_scrolled * ZOOM_SENSITIVITY;

        const {
            debug_assert!(MAX_SCALE_MANIPULATION >= 0.1 && MAX_SCALE_MANIPULATION <= 0.9);
        }

        let max_scale_manipulation = self.scales[0] * MAX_SCALE_MANIPULATION;
        self.user_scale = self
            .user_scale
            .clamp(-max_scale_manipulation / 2.0, max_scale_manipulation * 2.0);
    }

    /// Applies `delta` mouse movement as rotations along all the three axis.
    ///
    /// Mouse motion in X axis (`delta[0]`), rotates around Y axis.
    /// Mouse motion in Y axis (`delta[1]`), rotates around X axis.
    pub fn rotate(&mut self, delta: [f32; 3]) {
        let rotations = [
            delta[1] * ORBIT_SENSITIVITY,
            delta[0] * ORBIT_SENSITIVITY,
            delta[2] * ORBIT_SENSITIVITY,
        ];

        self.orientation = self.orientation.rotate(rotations);
        self.view = None;
    }

    /// Resets X & Y axis translations to `0.0`.
    pub fn reset_translations(&mut self) {
        self.translations[0] = 0.0;
        self.translations[1] = 0.0;
    }

    /// Resets additional X & Y axis scales to `0.0`.
    pub fn reset_scale(&mut self) {
        self.user_scale = 0.0
    }

    /// Returns the final scaling factors for each axis, after applying any additional scalings.
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
                .orientation // apply on top of current orientation
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

/// Calculates the edge of the cube that can contain any orientation of `stock_size`.
///
/// This will be a cube whose edge is equal to the
/// **space diagonal** of a cuboid of size `stock_size`.
fn max_bounding_cube_edge(stock_size: [f32; 3]) -> f32 {
    let xy_diagonal_sqr = stock_size[0].powi(2) + stock_size[1].powi(2);

    (xy_diagonal_sqr + stock_size[2].powi(2)).sqrt()
}

/// Computes the scaling factor, in **pixels per stock unit**
/// that fits a cube of `bounding_cube_edge` inside the window.
///
/// This can be used as X and Y scaling factor,
/// since both have a final range of `-1.0` to `1.0` in NDC.
fn xy_scale(window_size: [f32; 2], bounding_cube_edge: f32) -> f32 {
    if window_size[0] < window_size[1] {
        // scale to fit in x
        window_size[0] / bounding_cube_edge
    } else {
        window_size[1] / bounding_cube_edge
    }
}

/// Computes the scaling factor, in **NDC per stock unit**
/// that fits `bounding_cube_edge` in `0.5` NDC.
///
/// This can be used as Z scaling factor, which has a final range of `1.0` to `0.0` in NDC.
/// The scale only targets half that slice to provide **depth padding** of `0.25` on both extremes.
fn z_scale(bounding_cube_edge: f32) -> f32 {
    0.5 / bounding_cube_edge
}
