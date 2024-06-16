use std::{f32::consts::PI, mem, time::Duration};

use ira::{
	glam::{Quat, Vec3},
	physics::InstanceHandle,
	winit::{error::EventLoopError, window::Window},
	Context, Game, Instance, KeyCode, RigidBodyBuilder,
};
use ira_drum::Drum;

#[derive(Debug)]
struct Player {
	instance: InstanceHandle,

	d_position: Vec3,
	pitch: f32,
	yaw: f32,

	speed: f32,
	sensitivity: f32,
}

impl Player {
	pub fn new(instance: InstanceHandle) -> Self {
		Self {
			instance,
			d_position: Vec3::ZERO,
			pitch: 0.0,
			yaw: 0.0,
			speed: 10.0,
			sensitivity: 0.01,
		}
	}

	pub fn on_update(&mut self, ctx: &Context, delta: Duration) {
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

		let (_, rotation) = self.pos_rot(ctx);

		let axis = rotation.to_scaled_axis();
		let forward = axis.cross(Vec3::Y).normalize();
		let right = forward.cross(Vec3::Y).normalize();

		self.d_position += forward * dir.z * self.speed * delta;
		self.d_position += right * dir.x * self.speed * delta;
		self.d_position.y += dir.y * self.speed * delta;

		let d = ctx.mouse_delta();

		self.pitch += d.y * self.sensitivity;
		self.yaw += d.x * self.sensitivity;

		// clamp pitch
		self.pitch = self.pitch.clamp(-PI * 0.5, PI * 0.5);
	}

	pub fn pos_rot(&self, ctx: &Context) -> (Vec3, Quat) {
		let (_, instance) = self.instance.resolve(&ctx.drum);
		let (pos, rot) = instance.pos_rot(&ctx.physics);

		(pos, rot)
	}

	pub fn take_delta_pos(&mut self) -> Vec3 {
		mem::take(&mut self.d_position)
	}
}

struct App {
	player: Player,
	cars: Vec<InstanceHandle>,
}

impl ira::App for App {
	fn on_init(_window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(ctx: &mut Context) -> Self {
		let car_id = ctx.drum.model_id("bottled_car").unwrap();
		let cars = (0..10)
			.map(|i| {
				ctx.add_instance(
					car_id,
					Instance::builder()
						.up(Vec3::Z)
						.rotation(Quat::from_rotation_x(PI * 0.25))
						.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
						.scale(Vec3::splat(1.0))
						.rigidbody(RigidBodyBuilder::dynamic()),
				)
			})
			.collect();

		ctx.add_instance(
			ctx.drum.model_id("boring_cube").unwrap(),
			Instance::builder()
				.position(Vec3::Y * -5.0)
				.scale(Vec3::new(100.0, 1.0, 100.0)),
		);

		let player = ctx.add_instance(
			ctx.drum.model_id("orb").unwrap(),
			Instance::builder()
				.position(Vec3::new(0.0, 5.0, 0.0))
				.up(Vec3::Z)
				.scale(Vec3::splat(0.01))
				.rigidbody(RigidBodyBuilder::dynamic().lock_rotations()),
		);

		Self {
			player: Player::new(player),
			cars,
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context) {
		let jump = ctx.pressed(KeyCode::Space);

		// handle player movement
		let (_, player) = self.player.instance.resolve_mut(&mut ctx.drum);
		let d_pos = self.player.take_delta_pos();

		if jump {
			player.body.update(&mut ctx.physics, |body| {
				body.add_force((Vec3::Y * 1.0).into(), true);
			});
		}

		let (pos, _) = player.pos_rot(&ctx.physics);
		let rot = Quat::from_rotation_y(self.player.yaw);

		player.move_and_rotate(&mut ctx.physics, pos + d_pos.normalize_or_zero(), rot);

		let (position, _) = self.player.pos_rot(ctx);
		let rot = Quat::from_rotation_x(self.player.pitch) * rot;

		ctx.camera.apply(position, rot);
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.player.on_update(ctx, delta);

		for car in &mut self.cars {
			car.update(ctx, |i, p| i.rotate_y(p, delta.as_secs_f32() * PI * 0.25));
		}
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::<App>::default().run()
}
