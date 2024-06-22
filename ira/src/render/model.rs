use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use wgpu::util::DeviceExt;

use crate::{physics::PhysicsState, Body, GpuDrum, GpuModel, Instance};

pub trait VertexExt {
	const ATTRIBUTES: [wgpu::VertexAttribute; 4];

	fn desc() -> wgpu::VertexBufferLayout<'static>;
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
}

impl GpuModel {
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
		if self.instances.len() != self.last_instance_count {
			self.recreate_instance_buffer(device);
			self.last_instance_count = self.instances.len();
		} else if self.dirty {
			queue.write_buffer(
				&self.instance_buffer,
				0,
				bytemuck::cast_slice(&self.instances),
			);
		}
	}

	pub(crate) fn draw_instanced<'r, 's: 'r>(
		&'s self,
		pass: &mut wgpu::RenderPass<'r>,
		drum: &'s GpuDrum,
		camera_bind_group: &'s wgpu::BindGroup,
		light_bind_group: &'s wgpu::BindGroup,
		brdf_bind_group: &'s wgpu::BindGroup,
		transparent: bool,
	) {
		let meshes = if transparent {
			&self.meshes.transparent
		} else {
			&self.meshes.opaque
		};

		pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

		for mesh in meshes {
			let mesh = mesh.resolve(&drum.meshes);
			let material = mesh.material.resolve(&drum.materials);

			pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
			pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
			pass.set_bind_group(0, &material.bind_group, &[]);
			pass.set_bind_group(1, camera_bind_group, &[]);
			pass.set_bind_group(2, light_bind_group, &[]);
			pass.set_bind_group(3, brdf_bind_group, &[]);
			pass.draw_indexed(0..mesh.num_indices, 0, 0..self.instances.len() as _);
		}
	}
}

impl Instance {
	pub(crate) const VERTICES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
		4 => Float32x4,
		5 => Float32x4,
		6 => Float32x4,
		7 => Float32x4,
	];

	#[must_use]
	pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<GpuInstance>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::VERTICES,
		}
	}

	pub(crate) fn to_gpu(&self, physics: &PhysicsState) -> GpuInstance {
		let (position, rotation) = match self.body {
			Body::Rigid(handle) => {
				let position = physics
					.rigid_bodies
					.get(handle)
					.map_or_else(Default::default, |r| *r.position());

				position.into()
			}
			Body::Static { position, rotation } => (position, rotation),
		};

		GpuInstance {
			model: Mat4::from_scale_rotation_translation(self.scale, rotation, position)
				.to_cols_array_2d(),
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
