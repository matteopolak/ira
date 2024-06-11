use bincode::{Decode, Encode};
#[cfg(feature = "bytemuck")]
use bytemuck::{Pod, Zeroable};

use crate::{handle::Handle, material::Material};

#[must_use]
#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode)]
#[cfg_attr(feature = "bytemuck", derive(Zeroable, Pod))]
pub struct Vec3 {
	pub x: f32,
	pub y: f32,
	pub z: f32,
}

impl Vec3 {
	pub fn new(x: f32, y: f32, z: f32) -> Self {
		Self { x, y, z }
	}
}

#[must_use]
#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode)]
#[cfg_attr(feature = "bytemuck", derive(Zeroable, Pod))]
pub struct Vec2 {
	pub x: f32,
	pub y: f32,
}

impl Vec2 {
	pub fn new(x: f32, y: f32) -> Self {
		Self { x, y }
	}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode)]
#[cfg_attr(feature = "bytemuck", derive(Zeroable, Pod))]
pub struct Vertex {
	pub position: Vec3,
	pub normal: Vec3,
	pub tex_coords: Vec2,
	pub tangent: Vec3,
}

#[derive(Debug, Encode, Decode)]
pub struct Mesh {
	pub vertices: Box<[Vertex]>,
	pub indices: Box<[u32]>,
	pub material: Handle<Material>,
}

#[derive(Debug, Encode, Decode)]
pub struct Model {
	pub meshes: Box<[Handle<Mesh>]>,
	pub materials: Box<[Handle<Material>]>,
	pub center: Vec3,
}
