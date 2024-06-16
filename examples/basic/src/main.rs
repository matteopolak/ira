use std::{f32::consts::PI, time::Duration};

use ira::{
	glam::{Quat, Vec3},
	physics::InstanceRef,
	winit::{error::EventLoopError, window::Window},
	Context, Game, Instance, RigidBodyBuilder,
};
use ira_drum::Drum;

#[derive(Debug)]
struct Player {
	instance: InstanceRef,
}

impl Player {
	pub fn new(instance: InstanceRef) -> Self {
		Self { instance }
	}
}

struct App {
	player: Player,
	cars: Vec<InstanceRef>,
}

impl ira::App for App {
	fn on_init(_window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(ctx: &mut Context) -> Self {
		let cars = (0..10)
			.map(|i| {
				ctx.add_instance(
					0,
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
			1,
			Instance::builder()
				.position(Vec3::Y * -5.0)
				.scale(Vec3::new(100.0, 1.0, 100.0)),
		);

		let player = ctx.add_instance(
			2,
			Instance::builder()
				.position(Vec3::new(0.0, 20.0, 0.0))
				.up(Vec3::Z)
				.scale(Vec3::splat(0.01))
				.rigidbody(RigidBodyBuilder::dynamic()),
		);

		Self {
			player: Player::new(player),
			cars,
		}
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		for car in &mut self.cars {
			car.update(ctx, |i, p| i.rotate_y(p, delta.as_secs_f32() * PI * 0.25));
		}
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::<App>::default().run()
}
