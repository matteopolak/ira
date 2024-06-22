use std::{mem, time::Duration};

use ira::{
	extra::camera::CameraController,
	glam::{Quat, Vec3},
	packet::{Packet, UpdateInstance},
	Context, Instance, KeyCode,
};

#[derive(Debug)]
pub struct Player {
	pub instance: Instance,
	d_position: Vec3,

	pub camera: CameraController,
	pub speed: f32,

	ticks_since_last_update: u32,
	last_position: Vec3,
	last_rotation: Quat,
}

impl Player {
	pub fn new(instance: Instance) -> Self {
		Self {
			instance,
			d_position: Vec3::ZERO,
			camera: CameraController::default(),
			speed: 5.0,

			ticks_since_last_update: 0,
			last_position: Vec3::ZERO,
			last_rotation: Quat::IDENTITY,
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

		ctx.render.camera.apply(
			pos + self.d_position + Vec3::Y * 0.1,
			self.camera.yaw,
			self.camera.pitch,
		);
	}

	pub fn on_fixed_update(&mut self, ctx: &mut Context) {
		let delta_pos = mem::take(&mut self.d_position);

		let (pos, _) = self.instance.body.pos_rot(&ctx.physics);
		let pos = pos + delta_pos;

		let rot = Quat::from_rotation_y(self.camera.yaw);

		self.instance
			.set_position_rotation(&mut ctx.physics, pos, rot);

		self.ticks_since_last_update += 1;

		if self.ticks_since_last_update < 2 {
			return;
		}

		if let Some(instance_id) = ctx.instance_id {
			if !pos.abs_diff_eq(self.last_position, 0.0001)
				|| !rot.abs_diff_eq(self.last_rotation, 0.0001)
			{
				self.last_position = pos;
				self.last_rotation = rot;
			} else {
				return;
			}

			ctx.send_packet(Packet::UpdateInstance {
				id: instance_id,
				delta: UpdateInstance {
					position: pos,
					rotation: rot,
					scale: None,
					body: None,
				},
			});

			self.ticks_since_last_update = 0;
		}
	}
}
