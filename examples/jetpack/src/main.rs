use std::{f32::consts::FRAC_PI_4, time::Duration};

use ira::{
	glam::{Quat, Vec3},
	physics::InstanceHandle,
	winit::error::EventLoopError,
	Context, Game, Instance, KeyCode, RigidBodyBuilder,
};
use ira_drum::Drum;

struct App {
	player: basic::Player,
	cars: Vec<InstanceHandle>,
}

impl ira::App for App {
	fn create_player() -> (u32, ira::InstanceBuilder) {
		(2, ira::Instance::builder())
	}

	fn on_init() -> Drum {
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
						.rotation(Quat::from_rotation_x(FRAC_PI_4))
						.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
						.scale(Vec3::splat(5.0))
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
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context) {
		self.player.on_fixed_update(ctx);

		let jump = ctx.pressed(KeyCode::Space);
		let player = &mut self.player.instance;

		if jump {
			player.body.update(&mut ctx.physics, |body| {
				body.apply_impulse((Vec3::Y * 0.2).into(), true);
			});
		}

		for car in &mut self.cars {
			car.update(ctx, |i, p| {
				i.rotate_y(p, 0.01);
			});
		}
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.player.on_update(ctx, delta);
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::<App>::default().run()
}
