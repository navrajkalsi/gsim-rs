//! # Cursors
//!
//! Creates custom cursors for displaying active user mouse interaction.

use winit::{
    event_loop::EventLoop,
    window::{BadImage, Cursor, CustomCursor},
};

/// Width and height of each icon image in pixels.
const ICON_SIZE: u16 = 32;

/// Icon image to choose for [`Cursor`] construction.
pub enum CursorShape {
    /// Default pointer.
    Default,

    /// Rotation cursor.
    Rotate,

    /// 2D movement cursor.
    Pan,

    /// 2D rotation cursor.
    Orbit,
}

/// Collection of pre-generated [`Cursor`]s for each [`CursorShape`].
pub struct Cursors([Cursor; 4]);

impl Cursors {
    /// Constructs [`Cursor`] for each [`CursorShape`] and stores them for later retrieval.
    ///
    /// Returns error, on failure to either `open` any asset or create a [`CustomCursor`] from the
    /// asset.
    pub fn build(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        let circle = image::open("assets/circle.png")?;
        let rotate = image::open("assets/rotate.png")?;
        let pan = image::open("assets/pan.png")?;
        let orbit = image::open("assets/orbit.png")?;

        Ok(Self([
            cursor_from_rgba(circle.into_rgba8().into_raw(), event_loop)?,
            cursor_from_rgba(rotate.into_rgba8().into_raw(), event_loop)?,
            cursor_from_rgba(pan.into_rgba8().into_raw(), event_loop)?,
            cursor_from_rgba(orbit.into_rgba8().into_raw(), event_loop)?,
        ]))
    }

    /// Returns a `cursor` of the provided `shape`.
    pub fn get(&self, shape: CursorShape) -> Cursor {
        self.0[shape as usize].clone()
    }
}

fn cursor_from_rgba(rgba: Vec<u8>, event_loop: &EventLoop<()>) -> Result<Cursor, BadImage> {
    Ok(Cursor::Custom(event_loop.create_custom_cursor(
        CustomCursor::from_rgba(rgba, ICON_SIZE, ICON_SIZE, ICON_SIZE / 2, ICON_SIZE / 2)?,
    )))
}
