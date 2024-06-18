use std::{f32::consts::FRAC_PI_2, time::Duration};

use glam::Vec3;

use crate::Context;

#[must_use]
#[derive(Debug)]
pub struct CameraController {
	pub pitch: f32,
	pub yaw: f32,
	pub sensitivity: f32,
}

impl Default for CameraController {
	fn default() -> Self {
		Self {
			pitch: 0.0,
			yaw: 0.0,
			sensitivity: 0.1,
		}
	}
}

impl CameraController {
	/// Sets the pitch of the camera controller.
	pub fn with_pitch(mut self, pitch: f32) -> Self {
		self.pitch = pitch;
		self
	}

	/// Sets the yaw of the camera controller.
	pub fn with_yaw(mut self, yaw: f32) -> Self {
		self.yaw = yaw;
		self
	}

	/// Sets the sensitivity of the camera controller.
	pub fn with_sensitivity(mut self, sensitivity: f32) -> Self {
		self.sensitivity = sensitivity;
		self
	}

	/// Updates the camera's rotation based on the mouse delta.
	/// This should be called every frame.
	pub fn on_update(&mut self, ctx: &Context, delta: Duration) {
		let delta = delta.as_secs_f32();

		let d = ctx.mouse_delta();

		self.yaw += d.x * self.sensitivity * delta;
		self.pitch -= d.y * self.sensitivity * delta;
		self.pitch = self.pitch.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
	}

	/// Transforms a direction vector into one aligned with the camera's rotation.
	/// The input vector should be in world space.
	///
	/// # Example
	///
	/// ```
	/// let camera = CameraController::default()
	///   .with_pitch(0.0)
	///   .with_yaw(FRAC_PI_2);
	///
	/// let dir = Vec3::Z;
	/// let transformed = camera.transform_dir(dir);
	///
	/// assert_eq!(transformed, Vec3::X);
	/// ```
	#[must_use]
	pub fn transform_dir(&self, dir: Vec3) -> Vec3 {
		let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
		let forward = Vec3::new(yaw_cos, 0.0, yaw_sin);
		let right = forward.cross(Vec3::Y);

		(forward * dir.z + right * dir.x).normalize_or_zero()
	}
}
