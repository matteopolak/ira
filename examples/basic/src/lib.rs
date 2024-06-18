use std::{mem, time::Duration};

use ira::{
	extra::camera::CameraController,
	glam::{Quat, Vec3},
	Context, Instance, KeyCode,
};

#[derive(Debug)]
pub struct Player {
	pub instance: Instance,
	d_position: Vec3,

	pub camera: CameraController,
	pub speed: f32,
}

impl Player {
	pub fn new(instance: Instance) -> Self {
		Self {
			instance,
			d_position: Vec3::ZERO,
			camera: CameraController::default(),
			speed: 5.0,
		}
	}

	pub fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.camera.on_update(ctx, delta);

		let delta = delta.as_secs_f32();
		let dir = {
			let mut dir = Vec3::ZERO;

			if ctx.pressed(KeyCode::KeyW) {
				dir.z += 1.0;
			}

			if ctx.pressed(KeyCode::KeyS) {
				dir.z -= 1.0;
			}

			if ctx.pressed(KeyCode::KeyA) {
				dir.x -= 1.0;
			}

			if ctx.pressed(KeyCode::KeyD) {
				dir.x += 1.0;
			}

			dir
		}
		.normalize_or_zero();

		self.d_position += self.camera.transform_dir(dir) * self.speed * delta;

		let (pos, _) = self.instance.body.pos_rot(&ctx.physics);

		ctx.camera.apply(
			pos + self.d_position + Vec3::Y * 0.1,
			self.camera.yaw,
			self.camera.pitch,
		);
	}

	pub fn on_fixed_update(&mut self, ctx: &mut Context) {
		let delta_pos = mem::take(&mut self.d_position);

		let (pos, _) = self.instance.body.pos_rot(&ctx.physics);
		let pos = pos + delta_pos;

		self.instance.set_position_rotation(
			&mut ctx.physics,
			pos,
			Quat::from_rotation_z(self.camera.yaw),
		);
	}
}
