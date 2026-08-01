use std::cmp::Ordering;

use crate::{View, config::ToolConfig, points::Point};
use winit::dpi::PhysicalSize;

/// Additional margin applied to the stock in percentage of the screen.
const STOCK_INSET: f32 = 2.5;

const COS30: f32 = 0.8660254;
const SIN30: f32 = 0.5;

/// Represents the constant data to be shared across all geometric instances, per frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// View matrix to center the scale and scale it to [`Self::window_size`].
    /// Multiplication with this matrix results in **pixel** units.
    projection: [[f32; 4]; 4],
    /// Size of the stock.
    /// The first three numbers correspond to X, Y, and Z axis travels respectively.
    /// The last value is used for alignment and is never used.
    stock_size: [f32; 4],
    /// Width and height of the surface.
    window_size: [f32; 2],
    /// Active [`View`].
    view: View,

    tool_len: f32,
    tool_size: f32,
}

impl Uniforms {
    /// Constructs a new [`Uniforms`] with `view` set to [`View::default`].
    pub fn new(
        window_size: PhysicalSize<u32>,
        stock_size: Point,
        default_tool: ToolConfig,
    ) -> Self {
        let window_size = [window_size.width as f32, window_size.height as f32];
        let stock_size = [stock_size.x, stock_size.y, stock_size.z, 0.0];
        let view = View::default();

        let stock_view = stock_view(stock_size.as_slice(), view);
        let scale = scale(window_size, stock_view);
        let offset = offset(stock_size, stock_view, scale, view);

        Self {
            projection: projection_matrix(view, scale, offset),
            stock_size,
            window_size,
            view,
            tool_len: default_tool.length,
            tool_size: default_tool.diameter,
        }
    }

    /// Recalculates [`Self::projection`] view matrix for a new `window_size`.
    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = [window_size.width as f32, window_size.height as f32];
        let stock_view = stock_view(self.stock_size.as_slice(), self.view);
        let scale = scale(self.window_size, stock_view);
        let offset = offset(self.stock_size, stock_view, scale, self.view);
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

    pub fn set_tool(&mut self, tool_config: ToolConfig) {
        self.tool_len = tool_config.length;
        self.tool_size = tool_config.diameter;

        // TODO
    }
}

/// Returns the size of a rectangular view that would be needed to fit a stock with `stock_size`,
/// rendered from the provided [`View`].
///
/// The returned size will be in the same units as `stock_size`.
fn stock_view(stock_size: &[f32], view: View) -> [f32; 2] {
    match view {
        // use projection of the bounding box to get final x and y
        View::Isometric => project_bounding_box(stock_size),
        // use x and y of the stock
        View::Top => [stock_size[0], stock_size[1]],
    }
}

/// Returns the size of a rectangular view that would be needed to fit a stock with `stock_size`,
/// rendered from [`View::Isometric`].
///
/// The returned size will be in the same units as `stock_size`.
fn project_bounding_box(stock_size: &[f32]) -> [f32; 2] {
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
fn scale(window_size: [f32; 2], stock_view: [f32; 2]) -> f32 {
    const {
        assert!(STOCK_INSET >= 0.0 && STOCK_INSET <= 25.0);
    }

    // y / x
    // compensate for any inset
    let usable_percentage = 1.0 - (STOCK_INSET * 2.0) / 100.0;
    let usable_width = usable_percentage * window_size[0];
    let usable_height = usable_percentage * window_size[1];

    let window_ratio = usable_height / usable_width;
    let stock_ratio = stock_view[1] / stock_view[0];

    match stock_ratio.total_cmp(&window_ratio) {
        // y of stock is smaller, scale to fit x of stock and shrink in y
        Ordering::Less => usable_width / stock_view[0],
        // choose any
        Ordering::Equal => usable_width / stock_view[0],
        // y of stock is larger, scale to fit y of stock and shrink in x
        Ordering::Greater => usable_height / stock_view[1],
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
fn projection_matrix(view: View, scale: f32, offset: [f32; 2]) -> [[f32; 4]; 4] {
    // the actual matrix would visually be the transpose of the return value, row first
    let x = 500.0;
    let y = 250.0;
    let z = 250.0;
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
