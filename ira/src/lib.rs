#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::default_trait_access)]

pub mod camera;
pub mod drum;
pub mod game;
pub mod light;
pub mod material;
pub mod model;
pub mod physics;
pub mod texture;

pub use camera::*;
pub use drum::*;
pub use game::{App, Game};
pub use light::*;
pub use material::*;
pub use model::*;
pub use texture::*;

pub use glam;
pub use ira_drum::*;
pub use pollster;
pub use wgpu;
pub use winit;
