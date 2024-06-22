#![allow(dead_code, unused_variables, unused_imports)]

use std::{
	f32::consts::FRAC_PI_4,
	time::{Duration, Instant},
};

use ira::{
	glam::{Quat, Vec3},
	physics::InstanceHandle,
	winit::error::EventLoopError,
	Context, Game, Instance, RigidBodyBuilder,
};
use ira_drum::Drum;

struct App {
	player: basic::Player,
	cars: Vec<InstanceHandle>,

	last_car_spawn: Instant,
}

impl ira::App for App {
	fn create_player(ctx: &mut Context) -> (u32, ira::InstanceBuilder) {
		(
			2,
			ira::Instance::builder()
				.rigidbody(RigidBodyBuilder::dynamic().lock_rotations())
				.scale(Vec3::splat(0.05))
				.collider(
					ctx.drum
						.model_by_name("orb")
						.unwrap()
						.bounds()
						.to_cuboid(Vec3::splat(0.05))
						.mass(0.5),
				),
		)
	}

	fn on_init() -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(ctx: &mut Context) -> Self {
		let car_id = ctx.drum.model_id("bottled_car").unwrap();

		#[cfg(feature = "server")]
		let cars = (0..10)
			.map(|i| {
				ctx.add_instance(
					car_id,
					Instance::builder()
						.up(Vec3::Z)
						.rotation(Quat::from_rotation_x(FRAC_PI_4))
						.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
						.scale(Vec3::splat(5.0))
						.rigidbody(RigidBodyBuilder::dynamic()),
				)
			})
			.collect();
		#[cfg(not(feature = "server"))]
		let cars = Vec::new();

		#[cfg(feature = "server")]
		ctx.add_instance(
			ctx.drum.model_id("boring_cube").unwrap(),
			Instance::builder()
				.position(Vec3::Y * -5.0)
				.scale(Vec3::new(100.0, 1.0, 100.0)),
		);

		let body_collider = ctx.add_rigidbody(
			RigidBodyBuilder::dynamic().lock_rotations(),
			ctx.drum
				.model_by_name("orb")
				.unwrap()
				.bounds()
				.to_cuboid(Vec3::splat(0.05))
				.mass(0.5),
		);

		Self {
			player: basic::Player::new(Instance::from(body_collider)),
			cars,
			last_car_spawn: Instant::now(),
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context) {
		self.player.on_fixed_update(ctx);

		#[cfg(feature = "server")]
		if self.last_car_spawn.elapsed() > Duration::from_secs(1) {
			self.last_car_spawn = Instant::now();

			let car_id = ctx.drum.model_id("bottled_car").unwrap();
			let car = ctx.add_instance(
				car_id,
				Instance::builder()
					.up(Vec3::Z)
					.rotation(Quat::from_rotation_x(FRAC_PI_4))
					.position(Vec3::new(0.0, 10.0, 0.0))
					.scale(Vec3::splat(5.0))
					.rigidbody(RigidBodyBuilder::dynamic()),
			);

			self.cars.push(car);
		}
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.player.on_update(ctx, delta);

		/*for car in &mut self.cars {
			car.update(ctx, |i, p| {
				i.rotate_y(p, 0.01);
			});
		}*/
	}
}

fn main() -> Result<(), EventLoopError> {
	tracing_subscriber::fmt::init();

	Game::<App>::default().run()
}
