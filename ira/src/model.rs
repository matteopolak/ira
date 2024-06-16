use std::{mem, ops::Range};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use ira_drum::Handle;
use wgpu::util::DeviceExt;

use crate::{physics::BoundingBox, GpuDrum, GpuMaterial, GpuMesh};

pub trait VertexExt {
	const ATTRIBUTES: [wgpu::VertexAttribute; 4];

	fn desc() -> wgpu::VertexBufferLayout<'static>;
	fn into_gpu(self) -> GpuVertex;
}

impl VertexExt for ira_drum::Vertex {
	const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x3];

	#[must_use]
	fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::ATTRIBUTES,
		}
	}

	#[must_use]
	fn into_gpu(self) -> GpuVertex {
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
pub struct GpuMeshHandles {
	pub opaque: Box<[Handle<GpuMesh>]>,
	pub transparent: Box<[Handle<GpuMesh>]>,
}

impl GpuMeshHandles {
	pub fn min_max(&self, drum: &GpuDrum) -> (Vec3, Vec3) {
		let mut min = Vec3::splat(f32::INFINITY);
		let mut max = Vec3::splat(f32::NEG_INFINITY);

		for mesh in self.opaque.iter().chain(self.transparent.iter()) {
			let mesh = mesh.resolve(&drum.meshes);

			min = min.min(mesh.min);
			max = max.max(mesh.max);
		}

		(min, max)
	}
}

impl From<ira_drum::MeshHandles> for GpuMeshHandles {
	fn from(meshes: ira_drum::MeshHandles) -> Self {
		Self {
			opaque: meshes.opaque.iter().map(|h| Handle::new(h.raw())).collect(),
			transparent: meshes
				.transparent
				.iter()
				.map(|h| Handle::new(h.raw()))
				.collect(),
		}
	}
}

#[derive(Debug)]
pub struct GpuModel {
	pub meshes: GpuMeshHandles,
	pub instance_buffer: wgpu::Buffer,

	last_instance_count: usize,

	// INVARIANT: `instances` and `instance_data` are the same length.
	pub(crate) instances: Vec<GpuInstance>,
	pub(crate) instance_data: Vec<Instance>,

	pub(crate) bounds: BoundingBox,
}

impl GpuModel {
	pub fn new(
		device: &wgpu::Device,
		drum: &GpuDrum,
		meshes: GpuMeshHandles,
		instances: Vec<Instance>,
	) -> Self {
		let gpu_instances = instances.iter().map(Instance::to_gpu).collect::<Vec<_>>();
		let (min, max) = meshes.min_max(drum);

		Self {
			meshes,
			instance_buffer: Self::create_instance_buffer(device, &gpu_instances),
			last_instance_count: instances.len(),
			instances: gpu_instances,
			instance_data: instances,

			bounds: BoundingBox::Cuboid { min, max },
		}
	}

	pub fn add_instance(&mut self, instance: Instance) {
		self.instances.push(instance.to_gpu());
		self.instance_data.push(instance);
	}

	pub fn update_instance<F>(&mut self, index: usize, update: F)
	where
		F: FnOnce(&mut Instance),
	{
		update(&mut self.instance_data[index]);
	}

	#[must_use]
	pub fn create_instance_buffer(
		device: &wgpu::Device,
		instances: &[GpuInstance],
	) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("instance buffer"),
			contents: bytemuck::cast_slice(instances),
			usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
		})
	}

	/// Recreates the instance buffer with the current instance data.
	pub fn recreate_instance_buffer(&mut self, device: &wgpu::Device) {
		self.instance_buffer = Self::create_instance_buffer(device, &self.instances);
	}

	/// Updates the instance buffer with the current instance data.
	///
	/// If the number of instances has changed, the buffer is recreated.
	pub fn update_instance_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
		if self.instances.len() == self.last_instance_count {
			queue.write_buffer(
				&self.instance_buffer,
				0,
				bytemuck::cast_slice(&self.instances),
			);
		} else {
			self.recreate_instance_buffer(device);
			self.last_instance_count = self.instances.len();
		}
	}
}

#[must_use]
#[derive(Debug)]
pub struct Instance {
	pub last_position: Vec3,

	pub position: Vec3,
	pub rotation: Quat,
	pub scale: Vec3,

	pub velocity: Vec3,
	pub gravity: bool,
}

impl Default for Instance {
	fn default() -> Self {
		Self {
			last_position: Vec3::ZERO,
			position: Vec3::ZERO,
			rotation: Quat::IDENTITY,
			scale: Vec3::ONE,

			velocity: Vec3::ZERO,
			gravity: false,
		}
	}
}

#[must_use]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuInstance {
	model: [[f32; 4]; 4],
}

impl GpuInstance {
	#[must_use]
	pub fn model(&self) -> Mat4 {
		Mat4::from_cols_array_2d(&self.model)
	}
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

	/// Sets the position of the instance.
	///
	/// Do not use this to update the position of the instance. Instead, use
	/// [`update_instance`](GpuModel::update_instance).
	pub fn with_position(mut self, position: Vec3) -> Self {
		self.last_position = position;
		self.position = position;
		self
	}

	/// Enables gravity for the instance.
	pub fn with_gravity(mut self) -> Self {
		self.gravity = true;
		self
	}

	/// Sets the velocity of the instance.
	pub fn with_velocity(mut self, velocity: Vec3) -> Self {
		self.velocity = velocity;
		self
	}

	pub fn with_scale(mut self, scale: Vec3) -> Self {
		self.scale = scale;
		self
	}

	/// Rotates the instance such that `up` rotates the instance to the up vector
	/// of the engine (which is `Vec3::Y`).
	///
	/// This is particularly useful when importing a model with a different up vector.
	/// The up vector used in Ira is [`Vec3::Y`].
	pub fn with_up(mut self, up: Vec3) -> Self {
		self.rotation = Quat::from_rotation_arc(up, Vec3::Y);
		self
	}

	pub fn with_rotation(mut self, rotation: Quat) -> Self {
		self.rotation = rotation * self.rotation;
		self
	}

	pub fn rotate_y(&mut self, rad: f32) {
		self.rotation = Quat::from_rotation_y(rad) * self.rotation;
	}

	pub fn to_gpu(&self) -> GpuInstance {
		GpuInstance {
			model: Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
				.to_cols_array_2d(),
		}
	}
}

pub trait ModelExt {
	fn into_gpu(self, device: &wgpu::Device, drum: &GpuDrum, instances: Vec<Instance>) -> GpuModel;
}

impl ModelExt for ira_drum::Model {
	fn into_gpu(self, device: &wgpu::Device, drum: &GpuDrum, instances: Vec<Instance>) -> GpuModel {
		GpuModel::new(device, drum, self.meshes.into(), instances)
	}
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
