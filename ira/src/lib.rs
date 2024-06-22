#![feature(iter_array_chunks)]
#![warn(clippy::pedantic)]
#![allow(
	clippy::unused_async,
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_precision_loss,
	clippy::module_name_repetitions,
	clippy::default_trait_access
)]
#![cfg_attr(
	not(all(feature = "client", feature = "server")),
	allow(
		dead_code,
		unused_variables,
		unused_imports,
		clippy::needless_pass_by_value
	)
)]

#[cfg(feature = "client")]
pub mod camera;
pub mod client;
pub mod drum;
pub mod extra;
pub mod game;
#[cfg(feature = "client")]
pub mod light;
pub mod material;
pub mod model;
pub mod packet;
pub mod physics;
#[cfg(feature = "client")]
pub(crate) mod render;
pub mod server;

#[cfg(feature = "client")]
pub use camera::*;
pub use drum::*;
pub use game::{App, Context, Game};
#[cfg(feature = "client")]
pub use light::*;
pub use material::*;
pub use model::*;

pub use glam;
pub use ira_drum::*;
pub use pollster;
pub use rapier3d::data::{Arena, Coarena, Index};
pub use rapier3d::prelude::*;
#[cfg(feature = "client")]
pub use winit;
#[cfg(feature = "client")]
pub use winit::keyboard::KeyCode;
