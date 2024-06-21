#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::default_trait_access)]

pub mod camera;
#[cfg(feature = "client")]
pub mod client;
pub mod drum;
pub mod extra;
pub mod game;
pub mod light;
pub mod material;
pub mod model;
pub mod packet;
pub mod physics;
pub mod render;
#[cfg(feature = "server")]
pub mod server;
pub(crate) mod texture;

pub use camera::*;
pub use drum::*;
pub use game::{App, Context, Game};
pub use light::*;
pub use material::*;
pub use model::*;
pub use texture::*;

pub use glam;
pub use ira_drum::*;
pub use pollster;
pub use rapier3d::data::{Arena, Coarena, Index};
pub use rapier3d::prelude::*;
pub use winit;
pub use winit::keyboard::KeyCode;
