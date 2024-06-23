use std::time::Duration;

use ira::{glam::Vec3, Context, Game, Instance, RigidBodyBuilder};
use ira_drum::Drum;

struct App {
	player: basic::Player,
}

impl ira::App for App {
	fn create_player(_ctx: &mut Context) -> (u32, ira::InstanceBuilder) {
		unimplemented!()
	}

	fn on_init() -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(ctx: &mut Context) -> Self {
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
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context) {
		self.player.on_fixed_update(ctx);
	}

	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {
		self.player.on_update(ctx, delta);
	}
}

fn main() -> Result<(), ira::game::Error> {
	Game::<App>::default().run()
}
