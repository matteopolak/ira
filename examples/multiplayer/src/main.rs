#![allow(dead_code, unused_variables, unused_imports)]

use std::{
	f32::consts::FRAC_PI_4,
	time::{Duration, Instant},
};

use ira::{
	glam::{Quat, Vec3},
	physics::InstanceHandle,
	ColliderBuilder, Context, Game, Instance, RigidBodyBuilder,
};
use ira_drum::Drum;

struct App {
	#[cfg(feature = "client")]
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
		let cars = (0..0)
			.map(|i| {
				ctx.add_instance(
					car_id,
					Instance::builder()
						.up(Vec3::Z)
						.rotation(Quat::from_rotation_x(FRAC_PI_4))
						.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
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
				.position(Vec3::Y * -10.0)
				.scale(Vec3::new(10.0, 1.0, 10.0)),
		);

		#[cfg(feature = "client")]
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
			#[cfg(feature = "client")]
			player: basic::Player::new(Instance::from(body_collider)),
			cars,
			last_car_spawn: Instant::now(),
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context) {
		#[cfg(feature = "client")]
		self.player.on_fixed_update(ctx);
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		#[cfg(feature = "client")]
		self.player.on_update(ctx, delta);

		#[cfg(feature = "client")]
		if ctx.pressed(ira::KeyCode::KeyC) && self.last_car_spawn.elapsed() > Duration::from_secs(1)
		{
			self.last_car_spawn = Instant::now();

			let car_id = ctx.drum.model_id("bottled_car").unwrap();

			ctx.add_instance(
				car_id,
				Instance::builder()
					.up(Vec3::Z)
					.rotation(Quat::from_rotation_x(FRAC_PI_4))
					.position(Vec3::new(0.0, 10.0, 0.0))
					.scale(Vec3::splat(10.0))
					.rigidbody(RigidBodyBuilder::dynamic()),
			);
		}
	}
}

fn main() -> Result<(), ira::game::Error> {
	tracing_subscriber::fmt::init();

	Game::<App>::default().run()
}
