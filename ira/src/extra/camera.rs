use std::{f32::consts::FRAC_PI_2, mem, time::Duration};

use glam::Vec3;

use crate::Context;

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
	pub fn on_update(&mut self, ctx: &Context, delta: Duration) {
		let delta = delta.as_secs_f32();

		let d = ctx.mouse_delta();

		self.yaw += d.x * self.sensitivity * delta;
		self.pitch -= d.y * self.sensitivity * delta;
		self.pitch = self.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
	}

	/// Transforms a direction vector into one aligned with the camera's rotation.
	#[must_use]
	pub fn transform_dir(&self, dir: Vec3) -> Vec3 {
		let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
		let forward = Vec3::new(yaw_cos, 0.0, yaw_sin);
		let right = forward.cross(Vec3::Y);

		(forward * dir.z + right * dir.x).normalize_or_zero()
	}
}
