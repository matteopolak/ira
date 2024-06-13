use std::{fmt, mem, ops::Range, path::Path};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};
use gltf::Gltf;
use ira_drum::Handle;
use wgpu::util::DeviceExt;

use crate::{
	ext::{GpuDrum, GpuTexture},
	texture,
};

#[derive(Debug)]
pub struct Vertex {
	pub position: Vec3,
	pub normal: Vec3,
	pub tex_coords: Vec2,
	pub tangent: Vec3,
}

impl Vertex {
	pub const VERTICES: [wgpu::VertexAttribute; 4] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x3];

	#[must_use]
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::VERTICES,
		}
	}

	#[must_use]
	pub fn new(position: Vec3, normal: Vec3, tex_coords: Vec2) -> Self {
		Self {
			position,
			normal,
			tex_coords,
			tangent: Vec3::ZERO,
		}
	}

	#[must_use]
	pub fn into_gpu(self) -> GpuVertex {
		GpuVertex {
			position: self.position.into(),
			normal: self.normal.into(),
			tex_coords: self.tex_coords.into(),
			tangent: self.tangent.into(),
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuVertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
	pub tex_coords: [f32; 2],
	pub tangent: [f32; 3],
}

#[derive(Debug)]
pub struct Meshes {
	pub opaque: Box<[Handle<GpuMesh>]>,
	pub transparent: Box<[Handle<GpuMesh>]>,
}

impl From<ira_drum::Meshes> for Meshes {
	fn from(meshes: ira_drum::Meshes) -> Self {
		Self {
			opaque: meshes
				.opaque
				.into_iter()
				.map(|h| Handle::new(h.raw()))
				.collect(),
			transparent: meshes
				.transparent
				.into_iter()
				.map(|h| Handle::new(h.raw()))
				.collect(),
		}
	}
}

#[derive(Debug)]
pub struct GpuModel {
	pub meshes: Meshes,
	pub instance_buffer: wgpu::Buffer,
	pub instances: Vec<GpuInstance>,
}

#[must_use]
#[derive(Debug, Default)]
pub struct Instance {
	pub position: Vec3,
	pub rotation: glam::Quat,
}

#[must_use]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuInstance {
	pub model: [[f32; 4]; 4],
}

impl Instance {
	const VERTICES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
		4 => Float32x4,
		5 => Float32x4,
		6 => Float32x4,
		7 => Float32x4,
	];

	#[must_use]
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<GpuInstance>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::VERTICES,
		}
	}

	pub fn new(position: Vec3, rotation: glam::Quat) -> Self {
		Self { position, rotation }
	}

	/// Creates an instance with the given position and up vector.
	///
	/// This is particularly useful when importing a model with a different up vector.
	/// The up vector used in Ira is [`Vec3::Y`].
	pub fn from_up(position: Vec3, up: Vec3) -> Self {
		Self {
			position,
			rotation: glam::Quat::from_rotation_arc(up, Vec3::Y),
		}
	}

	pub fn to_gpu(&self) -> GpuInstance {
		GpuInstance {
			model: (Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation))
				.to_cols_array_2d(),
		}
	}
}

#[derive(Debug)]
pub struct GpuMaterial {
	pub albedo: Handle<GpuTexture>,
	pub normal: Handle<GpuTexture>,
	pub metallic_roughness: Handle<GpuTexture>,
	pub ao: Handle<GpuTexture>,
	pub emissive: Handle<GpuTexture>,

	pub transparent: bool,
	pub bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
pub struct GpuMesh {
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub material: Handle<GpuMaterial>,

	pub num_indices: u32,
}

#[derive(Debug)]
pub struct GpuMeshes {
	pub opaque: Box<[GpuMesh]>,
	pub transparent: Box<[GpuMesh]>,
}

pub trait DrawModel<'a> {
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'a GpuMesh,
		material: &'a GpuMaterial,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
		light_bind_group: &'a wgpu::BindGroup,
		brdf_bind_group: &'a wgpu::BindGroup,
	);

	fn draw_model_instanced(
		&mut self,
		drum: &'a GpuDrum,
		model: &'a GpuModel,
		camera_bind_group: &'a wgpu::BindGroup,
		light_bind_group: &'a wgpu::BindGroup,
		brdf_bind_group: &'a wgpu::BindGroup,
		transparent: bool,
	);
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
	'b: 'a,
{
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'b GpuMesh,
		material: &'a GpuMaterial,
		instances: Range<u32>,
		camera_bind_group: &'b wgpu::BindGroup,
		light_bind_group: &'b wgpu::BindGroup,
		brdf_bind_group: &'b wgpu::BindGroup,
	) {
		self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		self.set_bind_group(0, &material.bind_group, &[]);
		self.set_bind_group(1, camera_bind_group, &[]);
		self.set_bind_group(2, light_bind_group, &[]);
		self.set_bind_group(3, brdf_bind_group, &[]);
		self.draw_indexed(0..mesh.num_indices, 0, instances);
	}

	fn draw_model_instanced(
		&mut self,
		drum: &'b GpuDrum,
		model: &'b GpuModel,
		camera_bind_group: &'b wgpu::BindGroup,
		light_bind_group: &'b wgpu::BindGroup,
		brdf_bind_group: &'b wgpu::BindGroup,
		transparent: bool,
	) {
		let meshes = if transparent {
			&model.meshes.transparent
		} else {
			&model.meshes.opaque
		};

		self.set_vertex_buffer(1, model.instance_buffer.slice(..));

		for mesh in meshes {
			let mesh = mesh.resolve(&drum.meshes);
			let material = mesh.material.resolve(&drum.materials);

			self.draw_mesh_instanced(
				mesh,
				material,
				0..model.instances.len() as u32,
				camera_bind_group,
				light_bind_group,
				brdf_bind_group,
			);
		}
	}
}
