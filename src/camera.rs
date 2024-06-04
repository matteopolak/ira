use std::{f32::consts::FRAC_PI_2, time::Duration};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;
use winit::{
	event::{ElementState, MouseScrollDelta},
	keyboard::KeyCode,
};

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
		Mat4::perspective_rh(self.fovy.to_radians(), self.aspect, self.znear, self.zfar)
	}
}

pub struct CameraBuilder {
	pub position: Vec3,
	pub yaw: f32,
	pub pitch: f32,

	pub projection: Projection,
}

impl CameraBuilder {
	#[must_use]
	pub fn new(width: f32, height: f32) -> Self {
		Self {
			position: Vec3::ZERO,
			yaw: 0.0,
			pitch: 0.0,
			projection: Projection {
				aspect: width / height,
				fovy: 40f32.to_radians(),
				znear: 0.1,
				zfar: 1000.0,
			},
		}
	}

	#[must_use]
	pub fn to_view_projection_matrix(&self) -> Mat4 {
		let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
		let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

		Mat4::look_to_rh(
			self.position,
			self.position + Vec3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw),
			Vec3::Y,
		) * self.projection.to_perspective_matrix()
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Camera {
	view_proj: [[f32; 4]; 4],
	view_pos: [f32; 3],
	_padding: u32,
}

impl Camera {
	#[must_use]
	pub fn new(camera: &CameraBuilder) -> Self {
		let vp_matrix = camera.to_view_projection_matrix();

		Self {
			view_proj: vp_matrix.to_cols_array_2d(),
			view_pos: camera.position.into(),
			_padding: 0,
		}
	}

	fn update_view_proj(&mut self, camera: &CameraBuilder) {
		let vp_matrix = camera.to_view_projection_matrix();

		self.view_pos = camera.position.into();
		self.view_proj = vp_matrix.to_cols_array_2d();
	}

	pub fn create_on_device(self, device: &wgpu::Device) -> GpuCamera {
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
	pub uniform: Camera,
	pub buffer: wgpu::Buffer,
	pub bind_group: wgpu::BindGroup,
	pub bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuCamera {
	pub fn update_view_proj(&mut self, camera: &CameraBuilder, queue: &wgpu::Queue) {
		self.uniform.update_view_proj(camera);

		queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
	}
}

pub struct CameraController {
	pub camera: GpuCamera,
	pub builder: CameraBuilder,

	dir: Vec3,
	rot: Vec2,

	speed: f32,
	scroll: f32,
	sensitivity: f32,

	pub mouse_pressed: bool,
}

impl CameraController {
	#[must_use]
	pub fn new(camera: GpuCamera, builder: CameraBuilder) -> Self {
		Self {
			camera,
			builder,
			dir: Vec3::ZERO,
			rot: Vec2::ZERO,
			speed: 5.0,
			scroll: 0.0,
			sensitivity: 0.003,
			mouse_pressed: false,
		}
	}

	pub fn process_keyboard(&mut self, key: KeyCode, state: ElementState) -> bool {
		let amount = if state.is_pressed() { 1.0 } else { 0.0 };

		match key {
			KeyCode::KeyW | KeyCode::ArrowUp => {
				self.dir.z = amount;
			}
			KeyCode::KeyS | KeyCode::ArrowDown => {
				self.dir.z = -amount;
			}
			KeyCode::KeyA | KeyCode::ArrowLeft => {
				self.dir.x = -amount;
			}
			KeyCode::KeyD | KeyCode::ArrowRight => {
				self.dir.x = amount;
			}
			KeyCode::Space => {
				self.dir.y = amount;
			}
			KeyCode::ShiftLeft => {
				self.dir.y = -amount;
			}
			_ => return false,
		};

		true
	}

	pub fn process_mouse(&mut self, delta: (f32, f32)) {
		if self.mouse_pressed {
			self.rot += Vec2::from(delta);
		}
	}

	pub fn process_scroll(&mut self, delta: &MouseScrollDelta) {
		match delta {
			MouseScrollDelta::LineDelta(_, y) => {
				self.scroll += y * 100.;
			}
			MouseScrollDelta::PixelDelta(pos) => {
				self.scroll += pos.y as f32;
			}
		}
	}

	pub fn update(&mut self, delta: Duration) {
		let dt = delta.as_secs_f32();

		let (yaw_sin, yaw_cos) = self.rot.x.sin_cos();
		let forward = Vec3::new(yaw_cos, 0.0, yaw_sin).normalize();
		let right = forward.cross(Vec3::Y).normalize();

		self.builder.position += self.dir.z * forward * self.speed * dt;
		self.builder.position += self.dir.x * right * self.speed * dt;

		let (pitch_sin, pitch_cos) = self.rot.y.sin_cos();
		let scrollward = Vec3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();

		self.builder.position += self.scroll * scrollward * self.speed * dt;
		self.scroll = 0.0;

		self.builder.position.y += self.dir.y * self.speed * dt;

		self.builder.yaw += self.rot.x.to_radians() * self.sensitivity * dt;
		self.builder.pitch += -self.rot.y.to_radians() * self.sensitivity * dt;

		self.rot = Vec2::ZERO;

		self.builder.pitch = self.builder.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
	}
}
