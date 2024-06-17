use std::{
	f32::consts::{FRAC_PI_2, PI},
	mem,
	time::{Duration, Instant},
};

use ira::{
	extra::camera::CameraController,
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

	camera: CameraController,
	speed: f32,
}

impl Player {
	pub fn new(instance: InstanceHandle) -> Self {
		Self {
			instance,
			d_position: Vec3::ZERO,
			camera: CameraController::default(),
			speed: 0.5,
		}
	}

	pub fn on_update(&mut self, ctx: &Context, delta: Duration) {
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

		if jump {
			player.body.update(&mut ctx.physics, |body| {
				body.apply_impulse((Vec3::Y * 0.5).into(), true);
			});
		}

		let (pos, _) = player.pos_rot(&ctx.physics);
		let pos = pos + self.player.take_delta_pos();

		player.set_position_rotation(
			&mut ctx.physics,
			pos,
			Quat::from_rotation_z(self.player.camera.yaw),
		);

		for car in &mut self.cars {
			car.update(ctx, |i, p| {
				i.rotate_y(p, 0.01);
			});
		}
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.player.on_update(ctx, delta);

		let (_, player) = self.player.instance.resolve(&ctx.drum);
		let (pos, _) = player.pos_rot(&ctx.physics);

		ctx.camera.apply(
			pos + self.player.d_position,
			self.player.camera.yaw,
			self.player.camera.pitch,
		);
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::<App>::default().run()
}
