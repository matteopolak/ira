#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::module_name_repetitions)]

pub mod camera;
pub mod light;
pub mod model;
pub mod texture;

pub use glam;
pub use wgpu;
