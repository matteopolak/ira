use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::model::Model;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Light {
	pub position: [f32; 3],
	_padding: u32,
	pub color: [f32; 3],
	pub intensity: f32,
}

impl Default for Light {
	fn default() -> Self {
		Self {
			position: [200.0; 3],
			_padding: 0,
			// warm yellow
			color: [1.0, 0.9, 0.8],
			intensity: 3.0,
		}
	}
}

impl Light {
	/// Creates a default Light, but the position is set to the
	/// centroid of the model, with a large y value.
	#[must_use]
	pub fn from_model(model: &Model) -> Self {
		let centroid = model.centroid;
		let position = centroid + Vec3::Y * 10_000.0;

		Self {
			position: position.into(),
			..Default::default()
		}
	}

	/// Creates a default light in a specific position.
	#[must_use]
	pub fn from_position(position: Vec3) -> Self {
		Self {
			position: position.into(),
			..Default::default()
		}
	}

	pub const LAYOUT_ENTRY: wgpu::BindGroupLayoutEntry = wgpu::BindGroupLayoutEntry {
		binding: 0,
		visibility: wgpu::ShaderStages::from_bits_truncate(
			wgpu::ShaderStages::VERTEX.bits() | wgpu::ShaderStages::FRAGMENT.bits(),
		),
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};

	pub const LAYOUT_DESC: wgpu::BindGroupLayoutDescriptor<'static> =
		wgpu::BindGroupLayoutDescriptor {
			entries: &[Self::LAYOUT_ENTRY],
			label: Some("light_uniform_bind_group_layout"),
		};

	pub fn create_bind_group(
		device: &wgpu::Device,
		uniform_buffer: &wgpu::Buffer,
	) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
		let layout = device.create_bind_group_layout(&Self::LAYOUT_DESC);
		let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: uniform_buffer.as_entire_binding(),
			}],
			label: Some("light_uniform_bind_group"),
		});

		(bind_group, layout)
	}

	#[must_use]
	pub fn create_uniform_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("light_uniform_buffer"),
			contents: bytemuck::cast_slice(&[*self]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}

	#[must_use]
	pub fn create_on_device(self, device: &wgpu::Device) -> GpuLight {
		let buffer = self.create_uniform_buffer(device);
		let (bind_group, bind_group_layout) = Self::create_bind_group(device, &buffer);

		GpuLight {
			uniform: self,
			buffer,
			bind_group,
			bind_group_layout,
		}
	}
}

pub struct GpuLight {
	pub uniform: Light,
	pub buffer: wgpu::Buffer,
	pub bind_group: wgpu::BindGroup,
	pub bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuLight {
	pub fn set_position(&mut self, position: [f32; 3], queue: &wgpu::Queue) {
		self.uniform.position = position;

		queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
	}
}
