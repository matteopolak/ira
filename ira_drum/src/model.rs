use std::{fmt, ops};

use bincode::{Decode, Encode};
use bytemuck::{Pod, Zeroable};

use crate::{handle::Handle, material::Material};

#[must_use]
#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode, Zeroable, Pod)]
pub struct Vec3 {
	pub x: f32,
	pub y: f32,
	pub z: f32,
}

impl Vec3 {
	pub const ZERO: Self = Self::splat(0.0);
	pub const X: Self = Self::new(1.0, 0.0, 0.0);
	pub const Y: Self = Self::new(0.0, 1.0, 0.0);
	pub const Z: Self = Self::new(0.0, 0.0, 1.0);

	pub const fn splat(value: f32) -> Self {
		Self::new(value, value, value)
	}

	pub const fn new(x: f32, y: f32, z: f32) -> Self {
		Self { x, y, z }
	}

	pub fn cross(self, rhs: Self) -> Self {
		Self::new(
			self.y * rhs.z - self.z * rhs.y,
			self.z * rhs.x - self.x * rhs.z,
			self.x * rhs.y - self.y * rhs.x,
		)
	}

	pub fn normalize(self) -> Self {
		let length = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
		self / length
	}
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3 {
	fn from(glam::Vec3 { x, y, z }: glam::Vec3) -> Self {
		Self::new(x, y, z)
	}
}

#[cfg(feature = "glam")]
impl From<Vec3> for glam::Vec3 {
	fn from(Vec3 { x, y, z }: Vec3) -> Self {
		glam::Vec3::new(x, y, z)
	}
}

impl From<Vec3> for [f32; 3] {
	fn from(Vec3 { x, y, z }: Vec3) -> [f32; 3] {
		[x, y, z]
	}
}

impl From<[f32; 3]> for Vec3 {
	fn from([x, y, z]: [f32; 3]) -> Self {
		Self::new(x, y, z)
	}
}

impl ops::DivAssign<f32> for Vec3 {
	fn div_assign(&mut self, rhs: f32) {
		self.x /= rhs;
		self.y /= rhs;
		self.z /= rhs;
	}
}

impl ops::AddAssign for Vec3 {
	fn add_assign(&mut self, rhs: Self) {
		self.x += rhs.x;
		self.y += rhs.y;
		self.z += rhs.z;
	}
}

impl ops::Sub for Vec3 {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
	}
}

impl ops::Add for Vec3 {
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
	}
}

impl ops::Mul<f32> for Vec3 {
	type Output = Self;

	fn mul(self, rhs: f32) -> Self {
		Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
	}
}

impl ops::Div<f32> for Vec3 {
	type Output = Self;

	fn div(self, rhs: f32) -> Self {
		Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
	}
}

#[must_use]
#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Zeroable, Pod)]
pub struct Vec2 {
	pub x: f32,
	pub y: f32,
}

impl Vec2 {
	pub const ZERO: Self = Self::splat(0.0);

	pub const fn splat(value: f32) -> Self {
		Self::new(value, value)
	}

	pub const fn new(x: f32, y: f32) -> Self {
		Self { x, y }
	}
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Vec2 {
	fn from(glam::Vec2 { x, y }: glam::Vec2) -> Self {
		Self::new(x, y)
	}
}

#[cfg(feature = "glam")]
impl From<Vec2> for glam::Vec2 {
	fn from(Vec2 { x, y }: Vec2) -> Self {
		glam::Vec2::new(x, y)
	}
}

impl From<[f32; 2]> for Vec2 {
	fn from([x, y]: [f32; 2]) -> Self {
		Self::new(x, y)
	}
}

impl From<Vec2> for [f32; 2] {
	fn from(Vec2 { x, y }: Vec2) -> [f32; 2] {
		[x, y]
	}
}

impl ops::Sub for Vec2 {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		Self::new(self.x - rhs.x, self.y - rhs.y)
	}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Encode, Decode, Zeroable, Pod)]
pub struct Vertex {
	pub position: Vec3,
	pub normal: Vec3,
	pub tex_coords: Vec2,
	pub tangent: Vec3,
}

impl Vertex {
	/// Creates a new vertex with a zeroed tangent.
	#[must_use]
	pub fn new(position: Vec3, normal: Vec3, tex_coords: Vec2) -> Self {
		Self {
			position,
			normal,
			tex_coords,
			tangent: Vec3::ZERO,
		}
	}
}

#[derive(Debug)]
pub struct MeshBuilder {
	pub vertices: Vec<Vertex>,
	pub indices: Vec<u32>,
	pub material: Handle<Material>,
}

impl MeshBuilder {
	#[must_use]
	pub fn build(self) -> Mesh {
		let mut mesh = Mesh {
			vertices: self.vertices.into_boxed_slice(),
			indices: self.indices.into_boxed_slice(),
			material: self.material,
		};

		mesh.compute_tangents();
		mesh
	}
}

#[derive(Encode, Decode)]
pub struct Mesh {
	pub vertices: Box<[Vertex]>,
	pub indices: Box<[u32]>,
	pub material: Handle<Material>,
}

impl fmt::Debug for Mesh {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Mesh")
			.field(
				"vertices",
				&format_args!("Box<[Vertex; {}]>", self.vertices.len()),
			)
			.field(
				"indices",
				&format_args!("Box<[u32; {}]>", self.indices.len()),
			)
			.field("material", &self.material)
			.finish()
	}
}

impl Mesh {
	#[must_use]
	pub fn new(vertices: Box<[Vertex]>, indices: Box<[u32]>, material: Handle<Material>) -> Self {
		let mut mesh = Self {
			vertices,
			indices,
			material,
		};

		mesh.compute_tangents();
		mesh
	}

	#[tracing::instrument]
	pub fn compute_tangents(&mut self) {
		// compute tangents and bitangents
		for i in 0..self.indices.len() / 3 {
			let i0 = self.indices[i * 3] as usize;
			let i1 = self.indices[i * 3 + 1] as usize;
			let i2 = self.indices[i * 3 + 2] as usize;

			let v0 = &self.vertices[i0];
			let v1 = &self.vertices[i1];
			let v2 = &self.vertices[i2];

			let delta_pos1 = v1.position - v0.position;
			let delta_pos2 = v2.position - v0.position;

			let delta_uv1 = v1.tex_coords - v0.tex_coords;
			let delta_uv2 = v2.tex_coords - v0.tex_coords;

			let tangent = if delta_uv1 == Vec2::ZERO || delta_uv2 == Vec2::ZERO {
				// Calculate a default tangent using the normal
				let normal = v0.normal;
				let auxiliary_vector = if normal.y.abs() > 0.99 {
					Vec3::X
				} else {
					Vec3::Y
				};

				normal.cross(auxiliary_vector).normalize()
			} else {
				let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
				(delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r
			};

			self.vertices[i0].tangent += tangent;
			self.vertices[i1].tangent += tangent;
			self.vertices[i2].tangent += tangent;
		}

		for v in &mut self.vertices {
			v.tangent = v.tangent.normalize();
		}
	}
}

#[derive(Debug, Encode, Decode)]
pub struct MeshHandles {
	pub opaque: Box<[Handle<Mesh>]>,
	pub transparent: Box<[Handle<Mesh>]>,
}

#[derive(Debug, Encode, Decode)]
pub struct Model {
	pub meshes: MeshHandles,
	pub center: Vec3,
}
