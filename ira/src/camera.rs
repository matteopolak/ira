use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub struct Projection {
	pub aspect: f32,
	pub fovy: f32,
	pub znear: f32,
	pub zfar: f32,
}

impl Projection {
	pub fn resize(&mut self, width: f32, height: f32) {
		self.aspect = width / height;
	}

	#[must_use]
	pub fn to_perspective_matrix(&self) -> Mat4 {
		Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar)
	}
}

pub struct Settings {
	pub position: Vec3,
	pub yaw: f32,
	pub pitch: f32,

	pub projection: Projection,
}

impl Settings {
	#[must_use]
	pub fn new(width: f32, height: f32) -> Self {
		Self {
			position: Vec3::new(-2.0, 0.0, 0.0),
			yaw: 0.0,
			pitch: 0.0,
			projection: Projection {
				aspect: width / height,
				fovy: 80f32.to_radians(),
				znear: 0.1,
				zfar: 1000.0,
			},
		}
	}

	#[must_use]
	pub fn to_view_projection_matrix(&self) -> Mat4 {
		let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
		let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

		self.projection.to_perspective_matrix()
			* Mat4::look_to_rh(
				self.position,
				Vec3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize(),
				Vec3::Y,
			)
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
	projection: [[f32; 4]; 4],
	position: [f32; 3],
	_padding: u32,
}

impl Default for CameraUniform {
	fn default() -> Self {
		Self {
			projection: Mat4::IDENTITY.to_cols_array_2d(),
			position: Vec3::ZERO.into(),
			_padding: 0,
		}
	}
}

impl CameraUniform {
	#[must_use]
	pub fn from_settings(settings: &Settings) -> Self {
		Self {
			projection: settings.to_view_projection_matrix().to_cols_array_2d(),
			position: settings.position.into(),
			_padding: 0,
		}
	}

	pub fn into_gpu(self, device: &wgpu::Device) -> GpuCamera {
		let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera Buffer"),
			contents: bytemuck::cast_slice(&[self]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let camera_bind_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}],
				label: Some("camera_bind_group_layout"),
			});

		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &camera_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: camera_buffer.as_entire_binding(),
			}],
			label: Some("camera_bind_group"),
		});

		GpuCamera {
			uniform: self,
			buffer: camera_buffer,
			bind_group: camera_bind_group,
			bind_group_layout: camera_bind_group_layout,
		}
	}
}

#[must_use]
pub struct GpuCamera {
	pub(crate) uniform: CameraUniform,

	pub buffer: wgpu::Buffer,
	pub bind_group: wgpu::BindGroup,
	pub bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuCamera {
	fn update(&mut self, queue: &wgpu::Queue, rotation: Mat4, position: Vec3) {
		self.uniform.position = position.into();
		self.uniform.projection = rotation.to_cols_array_2d();

		queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
	}

	pub fn recreate_bind_group(&mut self, device: &wgpu::Device) {
		self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &self.bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: self.buffer.as_entire_binding(),
			}],
			label: Some("camera_bind_group"),
		});
	}
}

pub struct Camera {
	pub gpu: GpuCamera,
	pub settings: Settings,
}

impl Camera {
	#[must_use]
	pub fn new(gpu: GpuCamera, settings: Settings) -> Self {
		Self { gpu, settings }
	}

	pub fn update_view_proj(&mut self, queue: &wgpu::Queue) {
		self.gpu.update(
			queue,
			self.settings.to_view_projection_matrix(),
			self.settings.position,
		);
	}

	pub fn apply(&mut self, position: Vec3, yaw: f32, pitch: f32) {
		self.settings.position = position;
		self.settings.yaw = yaw;
		self.settings.pitch = pitch;
	}
}
