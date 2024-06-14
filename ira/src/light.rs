use glam::Vec3;
use ira_drum::Model;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
	pub position: [f32; 3],
	_padding: u32,
	pub color: [f32; 3],
	pub intensity: f32,
}

impl Default for GpuLight {
	fn default() -> Self {
		Self {
			position: [200.0; 3],
			_padding: 0,
			// warm yellow
			color: [1.0, 0.9, 0.8],
			intensity: 10.0,
		}
	}
}

#[must_use]
#[derive(Debug)]
pub struct Lights {
	pub lights: Vec<GpuLight>,
	pub uniform_buffer: wgpu::Buffer,
	pub bind_group: wgpu::BindGroup,
	pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Lights {
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

	pub fn from_lights(lights: &[GpuLight], device: &wgpu::Device) -> Self {
		let uniform_buffer = Self::create_uniform_buffer(lights, device);
		let (bind_group, bind_group_layout) = Self::create_bind_group(device, &uniform_buffer);

		Self {
			lights: lights.to_vec(),
			uniform_buffer,
			bind_group,
			bind_group_layout,
		}
	}

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
	pub fn create_uniform_buffer(lights: &[GpuLight], device: &wgpu::Device) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("light_uniform_buffer"),
			contents: bytemuck::cast_slice(lights),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}
}

impl GpuLight {
	/// Creates a default Light, but the position is set to the
	/// centroid of the model, with a large y value.
	#[must_use]
	pub fn from_model(model: &Model) -> Self {
		let centroid: Vec3 = model.center.into();
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
}
