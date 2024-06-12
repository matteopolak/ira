//! A container format for models and materials.
//!
//! Supported material properties:
//! - Diffuse color (required)
//! - Normal map
//! - Roughness map
//! - Metallic map
//! - Ambient occlusion map
//!
//! Supported model properties:
//! - Vertices
//! - Indices
//! - Normals
//! - Texture coordinates
//! - Tangents
//! - Bitangents
//! - Material index

#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::enum_glob_use)]
// TODO: Remove these once the `as` casts are fixed.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]

pub mod drum;
pub mod handle;
pub mod material;
pub mod model;

pub use drum::*;
pub use handle::*;
pub use material::*;
pub use model::*;
