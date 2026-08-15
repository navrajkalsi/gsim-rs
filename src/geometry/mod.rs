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

pub mod line;
mod math;
pub mod stock;
pub mod tools;
pub mod uniforms;
pub mod view;
