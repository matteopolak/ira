use std::{f32::consts::PI, time::Duration};

use ira::{
	game::State,
	glam::{Quat, Vec3},
	physics::InstanceRef,
	winit::{error::EventLoopError, window::Window},
	Game, InstanceBuilder, RigidBodyBuilder,
};
use ira_drum::Drum;

#[derive(Debug)]
struct Player {
	_instance: InstanceRef,
}

impl Default for Player {
	fn default() -> Self {
		Self {
			_instance: InstanceRef::new(2, 0),
		}
	}
}

#[derive(Default)]
struct App {
	_player: Player,
}

impl ira::App for App {
	fn init(&mut self, _window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(&mut self, state: &mut State) {
		// car
		for i in 0..10 {
			state.add_instance(
				0,
				InstanceBuilder::default()
					.up(Vec3::Z)
					.rotation(Quat::from_rotation_x(PI * 0.25))
					.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
					.scale(Vec3::splat(5.0))
					.rigidbody(RigidBodyBuilder::dynamic()),
			);
		}

		// floor
		state.add_instance(
			1,
			InstanceBuilder::default().scale(Vec3::new(100.0, 0.1, 100.0)),
		);

		// player model
		/*state.add_instance(
			2,
			InstanceBuilder::default()
				.position(Vec3::new(0.0, 10.0, 0.0))
				.up(Vec3::Z)
				.rigidbody(RigidBodyBuilder::dynamic()),
		);*/
	}

	fn on_frame(&mut self, state: &mut State, delta: Duration) {
		let model = &mut state.drum.models[0];

		model.update_instance(0, |instance| {
			instance.rotate_y(delta.as_secs_f32() * PI * 0.25);
		});
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::new(App::default()).run()
}
