//! # Uniforms
//!
//! The CPU representation of the uniform buffer to be used in the shaders.

use crate::geometry::math::Matrix;

/// The cumulation of all [`tranformations`](crate::renderer::tranform) to be applied to the
/// simulation, represented as a single [`Matrix`].
///
/// Multiplying a position vector (in **machine units**) with this matrix,
/// gives the final position vector (in **NDC**).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms(pub Matrix);
