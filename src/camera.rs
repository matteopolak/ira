use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use winit::{
	event::{ElementState, KeyEvent, WindowEvent},
	keyboard::{KeyCode, PhysicalKey},
};

pub struct Camera {
	eye: Vec3,
	target: Vec3,
	up: Vec3,
	aspect_ratio: f32,
	fovy: f32,
	znear: f32,
	zfar: f32,
}

impl Camera {
	pub fn new(width: f32, height: f32) -> Self {
		Self {
			eye: Vec3::new(0.0, 10.0, 20.0),
			target: Vec3::ZERO,
			up: Vec3::Y,
			aspect_ratio: width / height,
			fovy: 40.0,
			znear: 0.1,
			zfar: 1000.0,
		}
	}

	pub fn build_view_projection_matrix(&self) -> Mat4 {
		let view = Mat4::look_at_rh(self.eye, self.target, self.up);
		let proj = Mat4::perspective_rh(
			self.fovy.to_radians(),
			self.aspect_ratio,
			self.znear,
			self.zfar,
		);

		proj * view
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
	view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
	pub fn new(camera: &Camera) -> Self {
		let vp_matrix = camera.build_view_projection_matrix();

		Self {
			view_proj: vp_matrix.to_cols_array_2d(),
		}
	}

	pub fn update_view_proj(&mut self, camera: &Camera) {
		let vp_matrix = camera.build_view_projection_matrix();
		self.view_proj = vp_matrix.to_cols_array_2d();
	}
}

pub struct CameraController {
	speed: f32,
	is_forward_pressed: bool,
	is_backward_pressed: bool,
	is_left_pressed: bool,
	is_right_pressed: bool,
}

impl CameraController {
	pub fn new(speed: f32) -> Self {
		Self {
			speed,
			is_forward_pressed: false,
			is_backward_pressed: false,
			is_left_pressed: false,
			is_right_pressed: false,
		}
	}

	pub fn process_events(&mut self, event: &WindowEvent) -> bool {
		match event {
			WindowEvent::KeyboardInput {
				event:
					KeyEvent {
						physical_key: PhysicalKey::Code(keycode),
						state,
						..
					},
				..
			} => {
				let is_pressed = *state == ElementState::Pressed;
				match keycode {
					KeyCode::KeyW | KeyCode::ArrowUp => {
						self.is_forward_pressed = is_pressed;
						true
					}
					KeyCode::KeyA | KeyCode::ArrowLeft => {
						self.is_left_pressed = is_pressed;
						true
					}
					KeyCode::KeyS | KeyCode::ArrowDown => {
						self.is_backward_pressed = is_pressed;
						true
					}
					KeyCode::KeyD | KeyCode::ArrowRight => {
						self.is_right_pressed = is_pressed;
						true
					}
					_ => false,
				}
			}
			_ => false,
		}
	}

	pub fn update_camera(&self, camera: &mut Camera) {
		let forward = camera.target - camera.eye;
		let forward_norm = forward.normalize();
		let forward_mag = forward.length();

		// Prevents glitching when the camera gets too close to the
		// center of the scene.
		if self.is_forward_pressed && forward_mag > self.speed {
			camera.eye += forward_norm * self.speed;
		}

		if self.is_backward_pressed {
			camera.eye -= forward_norm * self.speed;
		}

		let right = forward_norm.cross(camera.up);

		// Redo radius calc in case the forward/backward is pressed.
		let forward = camera.target - camera.eye;
		let forward_mag = forward.length();

		if self.is_right_pressed {
			// Rescale the distance between the target and the eye so
			// that it doesn't change. The eye, therefore, still
			// lies on the circle made by the target and eye.
			camera.eye = camera.target - (forward + right * self.speed).normalize() * forward_mag;
		}

		if self.is_left_pressed {
			camera.eye = camera.target - (forward - right * self.speed).normalize() * forward_mag;
		}
	}
}
