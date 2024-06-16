use std::{f32::consts::PI, time::Duration};

use ira::{
	game::State,
	glam::{Quat, Vec3},
	physics::InstanceRef,
	winit::{error::EventLoopError, window::Window},
	Game, Instance,
};
use ira_drum::Drum;

#[derive(Debug)]
struct Player {
	instance: InstanceRef,
}

impl Default for Player {
	fn default() -> Self {
		Self {
			instance: InstanceRef::new(2, 0),
			..Default::default()
		}
	}
}

struct App {
	player: Player,
}

impl ira::App for App {
	fn init(&mut self, _window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(&mut self, state: &mut State) {
		// car
		let car = &mut state.drum.models[0];

		car.add_instance(
			Instance::default()
				.with_up(Vec3::Z)
				.with_position(Vec3::new(0.0, 10.0, 0.0))
				.with_gravity(),
		);

		// floor
		let floor = &mut state.drum.models[1];

		floor.add_instance(Instance::default());

		// player model
		let player = &mut state.drum.models[2];

		player.add_instance(
			Instance::default()
				.with_position(Vec3::new(0.0, 10.0, 0.0))
				.with_up(Vec3::Z)
				.with_gravity(),
		);

		// set camera player
		state.controller.attach_to(InstanceRef::new(2, 0));
	}

	fn on_frame(&mut self, state: &mut State, delta: Duration) {
		let model = &mut state.drum.models[0];

		model.update_instance(0, |instance| {
			instance.rotate_y(delta.as_secs_f32() * PI * 0.25);
		});
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::new(App).run()
}
